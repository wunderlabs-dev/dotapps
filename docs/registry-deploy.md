# dotapps registry deployment

Deployed from `src/worker/` via `npx wrangler deploy` (account: wunderlabs).

| Item | Value |
| --- | --- |
| Base URL | `<redeployed in C0 integration>` |
| Publish token (`DOTAPPS_TOKEN`) | `<redeployed in C0 integration>` |
| R2 bucket | `dotapps-registry` |
| Blob transport | Fallback (`/v1/blob/*` via Worker) — presign secrets not set |

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
export DOTAPPS_REGISTRY=<redeployed in C0 integration>
export DOTAPPS_TOKEN=<redeployed in C0 integration>
curl -s $DOTAPPS_REGISTRY/v1/apps
```
