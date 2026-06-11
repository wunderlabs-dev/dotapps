# vibox registry deployment

Deployed from `src/worker/` via `npx wrangler deploy` (account: wunderlabs).

| Item | Value |
| --- | --- |
| Base URL | `https://vibox-registry.isopusoktoday.workers.dev` |
| Publish token (`VIBOX_TOKEN`) | `c8d60f03914af5004b75dd8261aec901` |
| R2 bucket | `vibox-registry` |
| Blob transport | Fallback (`/v1/blob/*` via Worker) — presign secrets not set |

The token is a throwaway demo credential, recorded here intentionally so
Phase C integration and the CLI can pick it up without a secret store.

To switch to presigned R2 URLs later (optional): create an R2 API token
(Object Read & Write on `vibox-registry`), then from `src/worker/`:

```bash
npx wrangler secret put R2_ACCESS_KEY_ID
npx wrangler secret put R2_SECRET_ACCESS_KEY
npx wrangler secret put ACCOUNT_ID   # 9314014021a0127a3f9bed9b4f77be3a
```

No redeploy needed; the Worker detects the secrets per-request and the
upload/download response shapes are identical in both modes.

## Quick usage

```bash
export VIBOX_REGISTRY=https://vibox-registry.isopusoktoday.workers.dev
export VIBOX_TOKEN=c8d60f03914af5004b75dd8261aec901
curl -s $VIBOX_REGISTRY/v1/apps
```
