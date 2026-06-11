#!/usr/bin/env bash
# Reset dotapps to a clean demo state: empty Library, fresh VM, app relaunched.
#
#   bash scripts/demo-reset.sh          # reset + relaunch the bundled .app
#   bash scripts/demo-reset.sh --no-open  # reset only, don't launch
#
# Only touches ~/.dotapps (the app's own data). Leaves the published registry
# untouched, so deep links keep working.
set -euo pipefail

APP="$(cd "$(dirname "$0")/.." && pwd)/target/release/bundle/macos/dotapps.app"
VM_DIR="$HOME/.dotapps/vm"
OPEN=1
[ "${1:-}" = "--no-open" ] && OPEN=0

echo "==> stopping dotapps and its VM"
# The bundled app runs .../dotapps.app/Contents/MacOS/opnble (opnble is the
# internal crate/binary name). Match the bundle path, not "target/release".
pkill -9 -f "dotapps.app/Contents/MacOS/opnble" 2>/dev/null || true
pkill -9 -f vfkit 2>/dev/null || true
pkill -9 -f gvproxy 2>/dev/null || true
sleep 2

echo "==> clearing installed apps and VM scratch state"
rm -f "$HOME/.dotapps/apps.json"
rm -rf "$HOME/.dotapps/repos"
rm -f "$VM_DIR/alpine.img" "$VM_DIR/vfkit-efi-store"
find "$VM_DIR" -name "*.sock" -delete 2>/dev/null || true
find "$VM_DIR" -name "*.pid" -delete 2>/dev/null || true

echo "==> ensuring VM base image is present"
if [ ! -f "$VM_DIR/alpine-base.img" ]; then
  bash "$(dirname "$0")/seed-vm-image.sh"
else
  echo "    base image already seeded"
fi

if [ "$OPEN" = "1" ]; then
  echo "==> launching dotapps.app (VM boots in ~10-15s)"
  open "$APP"
  echo "Ready. Library is empty; install with a deep link, e.g.:"
  echo "    open \"dotapps://cafe-tracker\""
else
  echo "Reset complete (app not launched)."
fi
