#!/usr/bin/env bash
# vm-doctor: clear stuck vfkit/gvproxy processes after `make kill-dev` fails.
#
# Targets macOS Hypervisor.framework deadlocks where SIGKILL is queued but
# the kernel cannot deliver it (state code "UE" or "?E": uninterruptible +
# exit pending). Escalates per pid:
#   1. SIGTERM
#   2. SIGKILL
#   3. lldb attach + "process detach --kill"
#   4. Mach task_terminate via Python ctypes
# Run with sudo if the unprivileged pass leaves anything; sudo lifts the
# task_for_pid restriction and lets lldb attach to system-owned helpers.
#
# Exit code: 0 if all targets cleared, 1 if any survived (reboot required).

set -u

WAIT_AFTER_SIGNAL=2

if [ -t 1 ]; then
  RED=$'\e[31m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; BLUE=$'\e[34m'
  DIM=$'\e[2m'; BOLD=$'\e[1m'; RESET=$'\e[0m'
else
  RED= GREEN= YELLOW= BLUE= DIM= BOLD= RESET=
fi

LAST_STEPS=""

snapshot() {
  ps -axo pid,stat,etime,command 2>/dev/null \
    | awk '
      NR == 1 { print; next }
      /\/(vfkit|gvproxy)( |$)/ { print }
    ' \
    | sed 's/^/  /'
}

stuck_pids() {
  ps -axo pid,stat,command 2>/dev/null \
    | awk '
      NR == 1 { next }
      /\/(vfkit|gvproxy)( |$)/ {
        stat = $2
        if (stat ~ /^[UZ?]/ || stat ~ /E/) print $1
      }
    '
}

is_alive() { kill -0 "$1" 2>/dev/null; }

try_signal() {
  kill -"$2" "$1" 2>/dev/null || true
  sleep "$WAIT_AFTER_SIGNAL"
}

try_lldb() {
  command -v lldb >/dev/null 2>&1 || return 0
  lldb --batch -p "$1" -o 'process detach --kill' >/dev/null 2>&1 || true
  sleep "$WAIT_AFTER_SIGNAL"
}

try_mach_terminate() {
  command -v python3 >/dev/null 2>&1 || return 0
  python3 - "$1" >/dev/null 2>&1 <<'PY' || true
import ctypes, ctypes.util, sys
libc = ctypes.CDLL(ctypes.util.find_library("c"))
libc.mach_task_self.restype = ctypes.c_uint
libc.task_for_pid.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.POINTER(ctypes.c_uint)]
libc.task_for_pid.restype = ctypes.c_int
libc.task_terminate.argtypes = [ctypes.c_uint]
libc.task_terminate.restype = ctypes.c_int
task = ctypes.c_uint(0)
if libc.task_for_pid(libc.mach_task_self(), int(sys.argv[1]), ctypes.byref(task)) != 0:
    sys.exit(1)
sys.exit(0 if libc.task_terminate(task.value) == 0 else 1)
PY
  sleep "$WAIT_AFTER_SIGNAL"
}

clear_one() {
  local pid="$1"
  LAST_STEPS=""
  is_alive "$pid" || return 0

  LAST_STEPS="TERM"
  try_signal "$pid" TERM
  is_alive "$pid" || return 0

  LAST_STEPS="$LAST_STEPS,KILL"
  try_signal "$pid" KILL
  is_alive "$pid" || return 0

  LAST_STEPS="$LAST_STEPS,LLDB"
  try_lldb "$pid"
  is_alive "$pid" || return 0

  LAST_STEPS="$LAST_STEPS,MACH"
  try_mach_terminate "$pid"
  is_alive "$pid" || return 0

  return 1
}

main() {
  printf '%svm-doctor%s\n\n' "$BOLD" "$RESET"
  echo "Before:"; snapshot; echo

  local stuck=()
  while IFS= read -r pid; do
    [ -n "$pid" ] && stuck+=("$pid")
  done < <(stuck_pids)
  local initial=${#stuck[@]} cleared=0 remaining

  if [ "$initial" -eq 0 ]; then
    printf '%sNo stuck vfkit/gvproxy processes.%s\n' "$GREEN" "$RESET"
    exit 0
  fi

  printf '%sFound %d stuck process(es). Escalating...%s\n' \
    "$YELLOW" "$initial" "$RESET"
  if [ "$(id -u)" != 0 ]; then
    printf '  %shint: rerun with sudo if anything survives%s\n' "$DIM" "$RESET"
  fi
  echo

  for pid in "${stuck[@]}"; do
    printf '  pid=%-7s ' "$pid"
    if clear_one "$pid"; then
      printf '%scleared%s via %s\n' "$GREEN" "$RESET" "$LAST_STEPS"
      cleared=$((cleared + 1))
    else
      printf '%sstill stuck%s after %s\n' "$RED" "$RESET" "$LAST_STEPS"
    fi
  done

  echo; echo "After:"; snapshot; echo
  remaining=$((initial - cleared))
  printf 'Cleared: %s%d%s   Remaining: %s%d%s\n' \
    "$GREEN" "$cleared" "$RESET" "$RED" "$remaining" "$RESET"

  if [ "$remaining" -gt 0 ]; then
    cat <<EOF

${YELLOW}${BOLD}Some processes survived every userspace escalation.${RESET}
They are stuck in macOS Hypervisor.framework uninterruptible wait. SIGKILL
is queued but the kernel cannot deliver it because vfkit is blocked on an
XPC reply from a Virtualization helper that already exited. Even Mach
task_terminate hits the same uninterruptible thread.

A reboot is the only known fix.

${DIM}Verify state codes: ps -axo pid,stat,command | grep -E '(vfkit|gvproxy)'
State "UE" or "?E" means SIGKILL is queued but undeliverable.${RESET}
EOF
    exit 1
  fi

  echo
  printf '%sCleaning orphaned sockets and lockfile...%s\n' "$BLUE" "$RESET"
  rm -f \
    "${HOME}/.opnble/vm/agent.sock" \
    "${HOME}/.opnble/vm/gvproxy-api.sock" \
    "${HOME}/.opnble/vm/gvproxy-vfkit.sock" \
    "${HOME}/.opnble/dev.lock" 2>/dev/null || true
  rm -f "${HOME}/.opnble/vm/vfkit-"*.sock 2>/dev/null || true

  printf '%sAll clear.%s\n' "$GREEN" "$RESET"
}

main "$@"
