interface Env {
  CF_API_TOKEN: string;
  CF_ACCOUNT_ID: string;
  CF_ZONE_ID: string;
  TUNNEL_DOMAIN: string;
  // R2 bucket binding for VM images. `wrangler.toml` wires this to the
  // `opnble-vm` bucket. Until the bucket exists the /vm/* routes return 500.
  VM_IMAGES: R2Bucket;
  // R2 bucket binding for app release artifacts (DMG, sig, latest.json).
  // `wrangler.toml` wires this to the `opnble-releases` bucket. The release
  // CI uploads to `releases/<tag>/<file>` and refreshes the channel pointers
  // at `releases/stable/latest.json` and `releases/beta/latest.json`.
  RELEASES: R2Bucket;
}

interface CreateTunnelRequest {
  projectName: string;
  projectId: string;
  hostPort: number;
}

interface CreateTunnelResponse {
  tunnelId: string;
  tunnelToken: string;
  url: string;
}

interface CfTunnelResult {
  id: string;
  token: string;
}

interface CfDnsRecord {
  id: string;
}

interface CfApiResponse<T> {
  result: T;
  success: boolean;
}

const CF_API = "https://api.cloudflare.com/client/v4";

const MAX_SUBDOMAIN_LENGTH = 30;
const SHORT_ID_BYTES = 4;
const TUNNEL_SECRET_BYTES = 32;

const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, DELETE, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

function sanitizeSubdomain(name: string): string {
  const sanitized = name
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, MAX_SUBDOMAIN_LENGTH);
  return sanitized || "project";
}

function shortId(): string {
  const bytes = new Uint8Array(SHORT_ID_BYTES);
  crypto.getRandomValues(bytes);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function jsonResponse(data: unknown, status: number = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json", ...CORS_HEADERS },
  });
}

interface GitHubUser {
  login: string;
  id: number;
}

async function checkAuth(request: Request): Promise<Response | null> {
  const auth = request.headers.get("Authorization");
  if (!auth || !auth.startsWith("Bearer ")) {
    return jsonResponse({ error: "unauthorized" }, 401);
  }

  const token = auth.slice("Bearer ".length);

  // Validate the token against GitHub's API
  const ghResponse = await fetch("https://api.github.com/user", {
    headers: {
      Authorization: `Bearer ${token}`,
      "User-Agent": "opnble-tunnel-worker",
      Accept: "application/vnd.github+json",
    },
  });

  if (!ghResponse.ok) {
    return jsonResponse({ error: "invalid GitHub token" }, 401);
  }

  // Token is valid, user is authenticated
  return null;
}

async function cfFetch(env: Env, path: string, method: string, body?: unknown): Promise<Response> {
  const options: RequestInit = {
    method,
    headers: {
      Authorization: `Bearer ${env.CF_API_TOKEN}`,
      "Content-Type": "application/json",
    },
  };
  if (body !== undefined) {
    options.body = JSON.stringify(body);
  }
  return fetch(`${CF_API}${path}`, options);
}

async function configureTunnelIngress(
  env: Env,
  tunnelId: string,
  hostname: string,
  hostPort: number,
): Promise<void> {
  const configRes = await cfFetch(
    env,
    `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel/${tunnelId}/configurations`,
    "PUT",
    {
      config: {
        ingress: [
          {
            hostname,
            service: `http://127.0.0.1:${hostPort}`,
            originRequest: {
              httpHostHeader: `localhost:${hostPort}`,
            },
          },
          { service: "http_status:404" },
        ],
      },
    },
  );
  if (!configRes.ok) {
    const text = await configRes.text();
    console.error("tunnel ingress configuration failed:", text);
    throw new Error("Tunnel ingress configuration failed");
  }
  const configData: CfApiResponse<unknown> = await configRes.json();
  if (!configData.success) {
    throw new Error("Tunnel ingress configuration returned unsuccessful response");
  }
}

async function createTunnel(env: Env, req: CreateTunnelRequest): Promise<CreateTunnelResponse> {
  const subdomain = `${sanitizeSubdomain(req.projectName)}-${shortId()}`;
  const hostname = `${subdomain}.${env.TUNNEL_DOMAIN}`;
  const tunnelName = `opnble-${req.projectId}-${subdomain}`;

  const secretBytes = new Uint8Array(TUNNEL_SECRET_BYTES);
  crypto.getRandomValues(secretBytes);
  const tunnelSecret = btoa(String.fromCharCode(...secretBytes));

  // Step 1: Create the tunnel
  const createRes = await cfFetch(env, `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel`, "POST", {
    name: tunnelName,
    tunnel_secret: tunnelSecret,
  });
  if (!createRes.ok) {
    const text = await createRes.text();
    console.error("tunnel creation failed:", text);
    throw new Error("Tunnel creation failed");
  }
  const tunnelData: CfApiResponse<CfTunnelResult> = await createRes.json();
  if (!tunnelData.success || !tunnelData.result) {
    throw new Error("Tunnel creation returned unsuccessful response");
  }
  const tunnelId = tunnelData.result.id;
  const tunnelToken = tunnelData.result.token;

  // Remotely managed tunnels ignore cloudflared's --url flag. Route traffic
  // through API-configured ingress instead (localhost:0 caused 502; missing
  // ingress causes 522 connection timed out).
  await configureTunnelIngress(env, tunnelId, hostname, req.hostPort);

  // Step 2: Create DNS CNAME record
  const dnsRes = await cfFetch(env, `/zones/${env.CF_ZONE_ID}/dns_records`, "POST", {
    type: "CNAME",
    name: subdomain,
    content: `${tunnelId}.cfargotunnel.com`,
    proxied: true,
    comment: `opnble tunnel for ${req.projectId}`,
  });
  if (!dnsRes.ok) {
    // Cleanup: delete the tunnel
    await cfFetch(env, `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel/${tunnelId}`, "DELETE");
    const text = await dnsRes.text();
    console.error("DNS record creation failed:", text);
    throw new Error("DNS record creation failed");
  }
  const dnsData: CfApiResponse<CfDnsRecord> = await dnsRes.json();
  if (!dnsData.success) {
    await cfFetch(env, `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel/${tunnelId}`, "DELETE");
    throw new Error("DNS record creation returned unsuccessful response");
  }

  return {
    tunnelId,
    tunnelToken,
    url: `https://${hostname}`,
  };
}

async function deleteTunnel(env: Env, tunnelId: string): Promise<void> {
  // Step 1: Find and delete DNS records pointing to this tunnel
  const dnsListRes = await cfFetch(
    env,
    `/zones/${env.CF_ZONE_ID}/dns_records?type=CNAME&content=${tunnelId}.cfargotunnel.com`,
    "GET",
  );
  if (dnsListRes.ok) {
    const dnsData: CfApiResponse<CfDnsRecord[]> = await dnsListRes.json();
    if (dnsData.success && dnsData.result) {
      for (const record of dnsData.result) {
        const delRes = await cfFetch(
          env,
          `/zones/${env.CF_ZONE_ID}/dns_records/${record.id}`,
          "DELETE",
        );
        if (!delRes.ok) {
          console.error(`DNS record ${record.id} deletion failed: ${String(delRes.status)}`);
        }
      }
    }
  } else {
    console.error(`DNS record list fetch failed: ${String(dnsListRes.status)}`);
  }

  // Step 2: Clean up active connections
  await cfFetch(env, `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel/${tunnelId}/connections`, "DELETE");

  // Step 3: Delete the tunnel
  const deleteRes = await cfFetch(
    env,
    `/accounts/${env.CF_ACCOUNT_ID}/cfd_tunnel/${tunnelId}`,
    "DELETE",
  );
  if (!deleteRes.ok) {
    const text = await deleteRes.text();
    console.error("tunnel deletion failed:", text);
    throw new Error("Tunnel deletion failed");
  }
  const deleteData: CfApiResponse<unknown> = await deleteRes.json();
  if (!deleteData.success) {
    console.error("tunnel deletion returned unsuccessful response");
    throw new Error("Tunnel deletion returned unsuccessful response");
  }
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

    // GET serves the body; HEAD serves the same headers without a body so
    // curl -I, browser preflight, and Cloudflare's own cache probes can
    // size-check artifacts. Both methods route the same; serveR2 handles
    // method-specific behavior (HEAD strips body, Range only on GET).
    const isReadMethod = request.method === "GET" || request.method === "HEAD";
    const rangeHeader = request.headers.get("Range");

    try {
      // /updates/latest.json - stable channel manifest. The release CI
      // refreshes `releases/stable/latest.json` in R2 after a successful
      // signed build. Short cache (60s) so promotions surface fast.
      if (isReadMethod && path === "/updates/latest.json") {
        return serveR2(env.RELEASES, "stable/latest.json", "application/json", 60, false, request.method, rangeHeader);
      }

      // /updates/beta/latest.json - beta channel manifest, refreshed by
      // the release CI when the tag matches `v*-beta.*`. 60s cache so
      // testers pick up beta promotions within a minute.
      if (isReadMethod && path === "/updates/beta/latest.json") {
        return serveR2(env.RELEASES, "beta/latest.json", "application/json", 60, false, request.method, rangeHeader);
      }

      // /releases/<tag>/<file> - release artifact (DMG, sig, MSI, etc.)
      // uploaded by the release CI to `releases/<tag>/<file>` in R2. Tags
      // are immutable so we serve with a 1-year immutable cache. Range
      // requests are honored so Tauri's updater can resume large bundle
      // downloads after a connection drop.
      if (isReadMethod && path.startsWith("/releases/")) {
        const key = path.replace(/^\/releases\//, "");
        if (!isSafeR2Key(key)) {
          return jsonResponse({ error: "invalid release path" }, 400);
        }
        const contentType = key.endsWith(".json")
          ? "application/json"
          : key.endsWith(".sig")
            ? "text/plain; charset=utf-8"
            : "application/octet-stream";
        return serveR2(env.RELEASES, key, contentType, 31536000, true, request.method, rangeHeader);
      }

      // /vm/manifest.json - VM image manifest. Small JSON document
      // describing the current image: version, download URL, SHA256 of
      // the compressed payload and the decompressed image, sizes,
      // minimum app version. Served from R2 (binding VM_IMAGES). The
      // app reads this at first launch (Phase 2 Rust client) to decide
      // whether to download or to skip when the local image matches.
      if (isReadMethod && path === "/vm/manifest.json") {
        return serveR2(env.VM_IMAGES, "manifest.json", "application/json", 300, false, request.method, rangeHeader);
      }

      // /vm/<file>.img.zst - compressed VM image asset. Long-cache
      // because the filename is version-stamped; a new image gets a new
      // filename + a fresh manifest. 1 year cache + immutable. Range
      // requests honored so image_downloader.rs can resume the 162 MB
      // payload after a network blip without restarting from 0.
      if (isReadMethod && path.startsWith("/vm/") && path.endsWith(".img.zst")) {
        const key = path.replace(/^\/vm\//, "");
        if (!isSafeR2Key(key)) {
          return jsonResponse({ error: "invalid vm path" }, 400);
        }
        return serveR2(env.VM_IMAGES, key, "application/octet-stream", 31536000, true, request.method, rangeHeader);
      }

      // GET /health - health check
      if (request.method === "GET" && path === "/health") {
        return jsonResponse({ status: "ok" });
      }

      // POST /tunnels - create a tunnel
      if (request.method === "POST" && path === "/tunnels") {
        const authError = await checkAuth(request);
        if (authError) return authError;

        let body: CreateTunnelRequest;
        try {
          body = await request.json();
        } catch {
          return jsonResponse({ error: "invalid JSON body" }, 400);
        }

        if (!body.projectName || !body.projectId) {
          return jsonResponse({ error: "projectName and projectId are required" }, 400);
        }
        if (
          typeof body.hostPort !== "number" ||
          !Number.isInteger(body.hostPort) ||
          body.hostPort < 1 ||
          body.hostPort > 65535
        ) {
          return jsonResponse({ error: "hostPort must be an integer between 1 and 65535" }, 400);
        }
        const result = await createTunnel(env, body);
        return jsonResponse(result, 201);
      }

      // DELETE /tunnels/:id - delete a tunnel
      const deleteMatch = path.match(/^\/tunnels\/([a-f0-9-]+)$/);
      if (request.method === "DELETE" && deleteMatch) {
        const authError = await checkAuth(request);
        if (authError) return authError;

        const tunnelId = deleteMatch[1];
        await deleteTunnel(env, tunnelId);
        return jsonResponse({ ok: true });
      }

      return jsonResponse({ error: "Not found" }, 404);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Internal error";
      return jsonResponse({ error: message }, 500);
    }
  },
};
