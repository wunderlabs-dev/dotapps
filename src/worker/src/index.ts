import { AwsClient } from "aws4fetch";

interface Env {
  // R2 bucket binding for the app registry. `wrangler.toml` wires this to
  // the `vibox-registry` bucket. Key layout:
  //   apps/{slug}/{version}/app.vibox      - the published artifact
  //   apps/{slug}/{version}/manifest.json  - that version's Manifest
  //   apps/{slug}/latest                   - JSON {"version":"1.0.0"} pointer
  REGISTRY: R2Bucket;
  // Shared secret authenticating publish endpoints (and fallback blob PUT).
  PUBLISH_TOKEN: string;
  // Optional S3-compatible credentials for the bucket. When all three are
  // present the Worker hands out presigned R2 URLs so blob bytes bypass the
  // Worker entirely; otherwise upload/download fall back to /v1/blob/*.
  R2_ACCESS_KEY_ID?: string;
  R2_SECRET_ACCESS_KEY?: string;
  ACCOUNT_ID?: string;
}

// App manifest, camelCase on the wire. This shape is a frozen contract
// shared with the CLI (publish body) and the launcher (store/install).
interface Manifest {
  name: string;
  slug: string;
  version: string;
  icon: string;
  internalPort: number;
  description: string;
}

const R2_BUCKET_NAME = "vibox-registry";

const SLUG_PATTERN = /^[a-z0-9-]+$/;
const VERSION_PATTERN = /^[a-zA-Z0-9._-]+$/;
const MAX_VERSION_LENGTH = 128;
const MIN_PORT = 1;
const MAX_PORT = 65535;
const PRESIGN_EXPIRY_SECONDS = 3600;
const APPS_LIST_CACHE_SECONDS = 30;
const BLOB_CACHE_SECONDS = 31536000;

const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, PUT, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

function jsonResponse(
  data: unknown,
  status: number = 200,
  extraHeaders: Record<string, string> = {},
): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json", ...CORS_HEADERS, ...extraHeaders },
  });
}

// Publish endpoints (and the fallback blob PUT) require the shared token.
// Returns an error Response to short-circuit with, or null when authorized.
function checkAuth(request: Request, env: Env): Response | null {
  if (!env.PUBLISH_TOKEN) {
    return jsonResponse({ error: "unauthorized" }, 401);
  }
  const auth = request.headers.get("Authorization");
  if (auth !== `Bearer ${env.PUBLISH_TOKEN}`) {
    return jsonResponse({ error: "unauthorized" }, 401);
  }
  return null;
}

function isValidVersion(version: string): boolean {
  if (version.length === 0 || version.length > MAX_VERSION_LENGTH) return false;
  if (version.includes("..")) return false;
  return VERSION_PATTERN.test(version);
}

// Validates a publish body against the frozen Manifest contract. Returns a
// human-readable error string, or null when the manifest is acceptable.
function validateManifest(manifest: Manifest, pathSlug: string): string | null {
  if (typeof manifest.name !== "string" || manifest.name.length === 0) {
    return "name is required";
  }
  if (typeof manifest.slug !== "string" || !SLUG_PATTERN.test(manifest.slug)) {
    return "slug must match ^[a-z0-9-]+$";
  }
  if (manifest.slug !== pathSlug) {
    return "manifest slug must match the slug in the URL path";
  }
  if (typeof manifest.version !== "string" || !isValidVersion(manifest.version)) {
    return "version must be a non-empty string of [a-zA-Z0-9._-]";
  }
  if (
    typeof manifest.internalPort !== "number" ||
    !Number.isInteger(manifest.internalPort) ||
    manifest.internalPort < MIN_PORT ||
    manifest.internalPort > MAX_PORT
  ) {
    return `internalPort must be an integer between ${MIN_PORT} and ${MAX_PORT}`;
  }
  if (manifest.icon !== undefined && typeof manifest.icon !== "string") {
    return "icon must be a string";
  }
  if (manifest.description !== undefined && typeof manifest.description !== "string") {
    return "description must be a string";
  }
  return null;
}

function blobKey(slug: string, version: string): string {
  return `apps/${slug}/${version}/app.vibox`;
}

function manifestKey(slug: string, version: string): string {
  return `apps/${slug}/${version}/manifest.json`;
}

function latestKey(slug: string): string {
  return `apps/${slug}/latest`;
}

// Path-traversal guard for R2 keys derived from URL segments. Cloudflare R2
// treats keys as opaque strings, so `..` does not escape a bucket, but we
// still reject suspicious segments so attempts surface as 400 rather than
// returning unrelated objects from neighboring prefixes.
function isSafeR2Key(key: string): boolean {
  if (key.length === 0 || key.length > 512) return false;
  if (key.startsWith("/")) return false;
  if (key.includes("..")) return false;
  if (key.includes("//")) return false;
  return /^[a-zA-Z0-9._\-/]+$/.test(key);
}

// S3-compatible client for presigning R2 URLs, or null when the optional
// credentials are not configured (the fallback /v1/blob/* routes apply then).
function presignClient(env: Env): AwsClient | null {
  if (!env.R2_ACCESS_KEY_ID || !env.R2_SECRET_ACCESS_KEY || !env.ACCOUNT_ID) {
    return null;
  }
  return new AwsClient({
    accessKeyId: env.R2_ACCESS_KEY_ID,
    secretAccessKey: env.R2_SECRET_ACCESS_KEY,
    service: "s3",
    region: "auto",
  });
}

async function presignBlobUrl(
  env: Env,
  aws: AwsClient,
  key: string,
  method: "GET" | "PUT",
): Promise<string> {
  const url = new URL(
    `https://${env.ACCOUNT_ID}.r2.cloudflarestorage.com/${R2_BUCKET_NAME}/${key}`,
  );
  url.searchParams.set("X-Amz-Expires", String(PRESIGN_EXPIRY_SECONDS));
  const signed = await aws.sign(new Request(url, { method }), {
    aws: { signQuery: true },
  });
  return signed.url;
}

async function readJson<T>(bucket: R2Bucket, key: string): Promise<T | null> {
  let obj: R2ObjectBody | null;
  try {
    obj = await bucket.get(key);
  } catch (err) {
    console.error("R2 get failed for", key, err);
    return null;
  }
  if (obj === null) return null;
  try {
    return await obj.json<T>();
  } catch {
    console.error("R2 object is not valid JSON:", key);
    return null;
  }
}

// POST /v1/apps/{slug}/versions - start a publish. Stores the manifest
// immediately and hands back where to PUT the blob and where to POST when
// the upload finishes. The response shape is identical in presigned and
// fallback modes; clients never know which transport they got.
async function handleCreateVersion(
  request: Request,
  env: Env,
  origin: string,
  slug: string,
): Promise<Response> {
  let manifest: Manifest;
  try {
    manifest = await request.json();
  } catch {
    return jsonResponse({ error: "invalid JSON body" }, 400);
  }

  const validationError = validateManifest(manifest, slug);
  if (validationError !== null) {
    return jsonResponse({ error: validationError }, 400);
  }

  const version = manifest.version;
  await env.REGISTRY.put(manifestKey(slug, version), JSON.stringify(manifest), {
    httpMetadata: { contentType: "application/json" },
  });

  const aws = presignClient(env);
  const uploadUrl = aws
    ? await presignBlobUrl(env, aws, blobKey(slug, version), "PUT")
    : `${origin}/v1/blob/${blobKey(slug, version)}`;
  const completeUrl = `${origin}/v1/apps/${slug}/versions/${version}/complete`;

  return jsonResponse({ uploadUrl, completeUrl });
}

// POST /v1/apps/{slug}/versions/{version}/complete - finish a publish.
// Verifies the blob actually landed, then flips the `latest` pointer.
async function handleCompleteVersion(
  env: Env,
  slug: string,
  version: string,
): Promise<Response> {
  const blob = await env.REGISTRY.head(blobKey(slug, version));
  if (blob === null) {
    return jsonResponse({ error: "blob not uploaded" }, 400);
  }
  await env.REGISTRY.put(latestKey(slug), JSON.stringify({ version }), {
    httpMetadata: { contentType: "application/json" },
  });
  return jsonResponse({ ok: true });
}

// GET /v1/apps - list every published app at its latest version. Slugs with
// a dangling pointer or missing manifest are skipped rather than failing
// the whole listing.
async function handleListApps(env: Env): Promise<Response> {
  const slugPrefixes: string[] = [];
  let cursor: string | undefined;
  do {
    const page = await env.REGISTRY.list({ prefix: "apps/", delimiter: "/", cursor });
    slugPrefixes.push(...page.delimitedPrefixes);
    cursor = page.truncated ? page.cursor : undefined;
  } while (cursor !== undefined);

  const apps: { manifest: Manifest }[] = [];
  for (const prefix of slugPrefixes) {
    const slug = prefix.slice("apps/".length, -1);
    const latest = await readJson<{ version: string }>(env.REGISTRY, latestKey(slug));
    if (latest === null || !isValidVersion(latest.version)) continue;
    const manifest = await readJson<Manifest>(
      env.REGISTRY,
      manifestKey(slug, latest.version),
    );
    if (manifest === null) continue;
    apps.push({ manifest });
  }

  return jsonResponse(
    { apps },
    200,
    { "Cache-Control": `public, max-age=${APPS_LIST_CACHE_SECONDS}` },
  );
}

// GET /v1/apps/{slug}/latest - resolve the latest version's manifest plus a
// download URL for its blob (presigned when credentials exist, Worker-served
// /v1/blob/* otherwise).
async function handleGetLatest(env: Env, origin: string, slug: string): Promise<Response> {
  const latest = await readJson<{ version: string }>(env.REGISTRY, latestKey(slug));
  if (latest === null || !isValidVersion(latest.version)) {
    return jsonResponse({ error: "app not found" }, 404);
  }
  const manifest = await readJson<Manifest>(env.REGISTRY, manifestKey(slug, latest.version));
  if (manifest === null) {
    return jsonResponse({ error: "app not found" }, 404);
  }

  const aws = presignClient(env);
  const downloadUrl = aws
    ? await presignBlobUrl(env, aws, blobKey(slug, latest.version), "GET")
    : `${origin}/v1/blob/${blobKey(slug, latest.version)}`;

  return jsonResponse({ manifest, downloadUrl });
}

// Parse a single-range RFC 7233 `Range: bytes=<start>-<end>` header into the
// shape R2's `bucket.get()` accepts. Returns null when the header is absent,
// multi-range, or syntactically invalid (the caller should ignore and serve
// the full body in that case rather than 416, matching what nginx does for
// malformed Range headers).
function parseRangeHeader(header: string | null): R2Range | null {
  if (!header) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(header.trim());
  if (!match) return null;
  const startStr = match[1];
  const endStr = match[2];
  if (startStr === "" && endStr === "") return null;
  if (startStr === "") {
    // `bytes=-N` -> last N bytes
    const n = Number.parseInt(endStr, 10);
    if (!Number.isFinite(n) || n <= 0) return null;
    return { suffix: n };
  }
  const start = Number.parseInt(startStr, 10);
  if (!Number.isFinite(start) || start < 0) return null;
  if (endStr === "") {
    return { offset: start };
  }
  const end = Number.parseInt(endStr, 10);
  if (!Number.isFinite(end) || end < start) return null;
  return { offset: start, length: end - start + 1 };
}

// Compute the concrete (offset, length) actually served given the request
// range and the object's total size. R2's `obj.range` returns the resolved
// range for `suffix`/open-ended requests; for explicit ranges we trust the
// inputs but clamp to the object size to stay within RFC 7233.
function resolvedRange(
  requested: R2Range,
  totalSize: number,
): { offset: number; length: number } | null {
  if ("suffix" in requested) {
    const suffix = Math.min(requested.suffix, totalSize);
    return { offset: totalSize - suffix, length: suffix };
  }
  const offset = requested.offset ?? 0;
  if (offset >= totalSize) return null;
  const available = totalSize - offset;
  const length = requested.length === undefined
    ? available
    : Math.min(requested.length, available);
  return { offset, length };
}

async function serveR2(
  bucket: R2Bucket,
  key: string,
  contentType: string,
  maxAgeSeconds: number,
  immutable: boolean = true,
  method: string = "GET",
  rangeHeader: string | null = null,
): Promise<Response> {
  const range = method === "GET" ? parseRangeHeader(rangeHeader) : null;
  const cacheControl = immutable
    ? `public, max-age=${maxAgeSeconds}, immutable`
    : `public, max-age=${maxAgeSeconds}`;

  // Range path takes a HEAD first so we can return a proper 416 with the
  // object's size in Content-Range, instead of relying on R2 to error
  // (it throws an unhelpful generic error for some bad ranges, which
  // would otherwise bubble up as a 500 to the client). The extra HEAD is
  // cheap and happens only on resume / partial-fetch requests.
  if (range !== null) {
    let metadata: R2Object | null;
    try {
      metadata = await bucket.head(key);
    } catch (err) {
      console.error("R2 head failed for", key, err);
      return jsonResponse({ error: "storage unavailable" }, 500);
    }
    if (metadata === null) {
      return new Response("not found", { status: 404 });
    }
    const served = resolvedRange(range, metadata.size);
    const baseHeaders: Record<string, string> = {
      "content-type": contentType,
      "cache-control": cacheControl,
      etag: metadata.httpEtag,
      "accept-ranges": "bytes",
    };
    if (served === null || served.length === 0) {
      return new Response("range not satisfiable", {
        status: 416,
        headers: { ...baseHeaders, "content-range": `bytes */${metadata.size}` },
      });
    }
    let body: R2ObjectBody | null;
    try {
      body = await bucket.get(key, { range });
    } catch (err) {
      console.error("R2 get with range failed for", key, err);
      return jsonResponse({ error: "storage unavailable" }, 500);
    }
    if (body === null) {
      return new Response("not found", { status: 404 });
    }
    return new Response(body.body, {
      status: 206,
      headers: {
        ...baseHeaders,
        "content-length": String(served.length),
        "content-range": `bytes ${served.offset}-${served.offset + served.length - 1}/${metadata.size}`,
      },
    });
  }

  // Non-range path. HEAD uses bucket.head() to skip the body fetch.
  let obj: R2Object | R2ObjectBody | null;
  try {
    obj = method === "HEAD" ? await bucket.head(key) : await bucket.get(key);
  } catch (err) {
    console.error("R2 get failed for", key, err);
    return jsonResponse({ error: "storage unavailable" }, 500);
  }
  if (obj === null) {
    return new Response("not found", { status: 404 });
  }
  const headers: Record<string, string> = {
    "content-type": contentType,
    "cache-control": cacheControl,
    etag: obj.httpEtag,
    "accept-ranges": "bytes",
    "content-length": String(obj.size),
  };
  if (method === "HEAD") {
    return new Response(null, { headers });
  }
  return new Response((obj as R2ObjectBody).body, { headers });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS_HEADERS });
    }

    const url = new URL(request.url);
    const path = url.pathname;
    const origin = url.origin;
    const isReadMethod = request.method === "GET" || request.method === "HEAD";

    try {
      // GET /health - health check
      if (request.method === "GET" && path === "/health") {
        return jsonResponse({ status: "ok" });
      }

      // GET /v1/apps - list all published apps
      if (request.method === "GET" && path === "/v1/apps") {
        return handleListApps(env);
      }

      // GET /v1/apps/{slug}/latest - latest manifest + download URL
      const latestMatch = path.match(/^\/v1\/apps\/([^/]+)\/latest$/);
      if (request.method === "GET" && latestMatch) {
        const slug = latestMatch[1];
        if (!SLUG_PATTERN.test(slug)) {
          return jsonResponse({ error: "invalid slug" }, 400);
        }
        return handleGetLatest(env, origin, slug);
      }

      // POST /v1/apps/{slug}/versions - start a publish (auth)
      const versionsMatch = path.match(/^\/v1\/apps\/([^/]+)\/versions$/);
      if (request.method === "POST" && versionsMatch) {
        const authError = checkAuth(request, env);
        if (authError) return authError;
        const slug = versionsMatch[1];
        if (!SLUG_PATTERN.test(slug)) {
          return jsonResponse({ error: "invalid slug" }, 400);
        }
        return handleCreateVersion(request, env, origin, slug);
      }

      // POST /v1/apps/{slug}/versions/{version}/complete - finish a publish (auth)
      const completeMatch = path.match(/^\/v1\/apps\/([^/]+)\/versions\/([^/]+)\/complete$/);
      if (request.method === "POST" && completeMatch) {
        const authError = checkAuth(request, env);
        if (authError) return authError;
        const slug = completeMatch[1];
        const version = completeMatch[2];
        if (!SLUG_PATTERN.test(slug)) {
          return jsonResponse({ error: "invalid slug" }, 400);
        }
        if (!isValidVersion(version)) {
          return jsonResponse({ error: "invalid version" }, 400);
        }
        return handleCompleteVersion(env, slug, version);
      }

      // /v1/blob/{key} - fallback blob transport when presigned URLs are not
      // configured. PUT requires the publish token; GET/HEAD are public
      // (blobs are content under version-stamped keys) and honor Range so
      // the launcher can resume large downloads.
      if (path.startsWith("/v1/blob/")) {
        const key = path.slice("/v1/blob/".length);
        if (!isSafeR2Key(key)) {
          return jsonResponse({ error: "invalid blob path" }, 400);
        }

        if (request.method === "PUT") {
          const authError = checkAuth(request, env);
          if (authError) return authError;
          if (request.body === null) {
            return jsonResponse({ error: "missing request body" }, 400);
          }
          await env.REGISTRY.put(key, request.body);
          return jsonResponse({ ok: true });
        }

        if (isReadMethod) {
          const contentType = key.endsWith(".json")
            ? "application/json"
            : "application/octet-stream";
          // Version-stamped artifact keys are immutable; the `latest`
          // pointer is not, but downloadUrl never points at it.
          const immutable = !key.endsWith(".json") && !key.endsWith("/latest");
          return serveR2(
            env.REGISTRY,
            key,
            contentType,
            immutable ? BLOB_CACHE_SECONDS : 60,
            immutable,
            request.method,
            request.headers.get("Range"),
          );
        }
      }

      return jsonResponse({ error: "Not found" }, 404);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Internal error";
      return jsonResponse({ error: message }, 500);
    }
  },
};
