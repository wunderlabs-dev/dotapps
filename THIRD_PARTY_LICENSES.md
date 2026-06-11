# Third-party software bundled with Opnble

Opnble ships the following Apache-2.0-licensed binaries as Tauri `externalBin`
sidecars inside the platform-native bundle. They are bundled in their
unmodified upstream form, then re-signed at build time with the Opnble
Developer ID certificate as part of the macOS notarization flow.

The version pins live in `scripts/prepare-resources.sh`. SHA256 hashes are
verified on download.

## vfkit

- Upstream: <https://github.com/crc-org/vfkit>
- License: Apache-2.0
- Copyright: 2021-2026 Red Hat, Inc.
- Role: Apple Virtualization.framework wrapper used to run the Alpine guest
  on Apple Silicon Macs.

## gvisor-tap-vsock / gvproxy

- Upstream: <https://github.com/containers/gvisor-tap-vsock>
- License: Apache-2.0
- Copyright: containers project contributors
- Role: user-mode network stack that bridges vfkit's virtio-net to the host
  loopback. Provides DHCP, DNS, and port forwarding for the VM.

## cloudflared

- Upstream: <https://github.com/cloudflare/cloudflared>
- License: Apache-2.0
- Copyright: Cloudflare, Inc.
- Role: spawns Cloudflare Quick Tunnels so a running project's dev server can
  be shared via a public HTTPS URL.

## License text

The full Apache License, Version 2.0 is available at
<https://www.apache.org/licenses/LICENSE-2.0>.

```
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
