# dotapps registry deployment

Deployed from `src/worker/` via `npx wrangler deploy` (account: wunderlabs).

| Item | Value |
| --- | --- |
| Base URL | `https://dotapps-registry.isopusoktoday.workers.dev` |
| Publish token (`DOTAPPS_TOKEN`) | `6bb809422c071d06d93fc2cf07068ffe` |
| R2 bucket | `dotapps-registry` |
| Blob transport | Fallback (`/v1/blob/*` via Worker) — presign secrets not set |
| Custom domain | `registry.dotapps.club` (bound; resolves once `dotapps.club` NS propagates) |

The token is a throwaway demo credential, recorded here intentionally so
Phase C integration and the CLI can pick it up without a secret store.

To switch to presigned R2 URLs later (optional): create an R2 API token
(Object Read & Write on `dotapps-registry`), then from `src/worker/`:

```bash
npx wrangler secret put R2_ACCESS_KEY_ID
npx wrangler secret put R2_SECRET_ACCESS_KEY
npx wrangler secret put ACCOUNT_ID   # 9314014021a0127a3f9bed9b4f77be3a
```

No redeploy needed; the Worker detects the secrets per-request and the
upload/download response shapes are identical in both modes.

## Quick usage

```bash
export DOTAPPS_REGISTRY=https://dotapps-registry.isopusoktoday.workers.dev
export DOTAPPS_TOKEN=6bb809422c071d06d93fc2cf07068ffe
curl -s $DOTAPPS_REGISTRY/v1/apps
```
