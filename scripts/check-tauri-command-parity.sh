#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTER_FILE="$ROOT_DIR/src/tauri/src/app/register.rs"

if [[ ! -f "$REGISTER_FILE" ]]; then
  echo "[parity] register file not found: $REGISTER_FILE" >&2
  exit 1
fi

# Extract shared commands from the specta_builder_with! macro body,
# plus platform-specific commands from each run_* function's invocation.
extract_commands() {
  local fn_name="$1"

  # Shared commands: everything inside macro_rules! specta_builder_with { ... }
  # Platform commands: everything inside specta_builder_with![ ... ] in the fn
  {
    # Shared commands from macro definition (between collect_commands![ and $($platform_cmd)*)
    awk '
      /macro_rules! specta_builder_with/ {in_macro=1}
      in_macro && /collect_commands!\[/ {inside=1; next}
      inside && /\$\(\$platform_cmd\)/ {exit}
      inside {print}
    ' "$REGISTER_FILE"

    # Platform commands from fn invocation (between specta_builder_with![ and ];)
    awk -v f="$fn_name" '
      $0 ~ "pub fn " f "\\(" {fn_scope=1}
      fn_scope && /specta_builder_with!\[/ {inside=1; next}
      inside && /\];/ {exit}
      inside {print}
    ' "$REGISTER_FILE"
  } \
    | sed -E 's,//.*$,,' \
    | tr -d ' ' \
    | grep -E '^[A-Za-z0-9_:]+,$' \
    | sed -E 's/,$//' \
    | sed -E 's/.*:://' \
    | sort -u
}

shared_expected() {
  cat <<'CMDS'
add_project
app_version
auth_token
check_data_integrity
check_for_update
check_provider_support
checkout_branch
clone_repository
delete_auth_token
export_diagnostics
github_list_repos
github_poll_oauth
github_start_oauth
github_user
import_project
install_project
install_update
is_cursor_installed
list_branches
list_projects
mcp_delete_snapshot
mcp_discard_fork
mcp_list_snapshots
mcp_recent_tool_invocations
mcp_rollback_project
mcp_rotate_token
mcp_status
open_project_in_cursor
project
provider_from_url
publish_project
pull_repository
quit_app
remove_project
restart_project
reveal_logs_folder
save_settings
settings
share_project
show_window
start_project
state
stop_project
store_auth_token
unpublish_project
unshare_project
update_project
validate_auth_token
CMDS
}

macos_expected() {
  {
    shared_expected
    cat <<'CMDS'
cancel_vm_image_download
check_setup_status
check_windows_setup
container_status
dotapps_install_app
dotapps_installed_apps
dotapps_open_app
dotapps_registry_apps
dotapps_reset
dotapps_run_app
dotapps_stop_app
download_vm_image
enable_wsl_windows
init_vm
init_wsl
is_vm_running
list_running_containers
reboot_windows
run_initial_setup
stop_vm
vm_exec
vm_image_status
vm_stats
vm_status
CMDS
  } | sort -u
}

windows_expected() {
  {
    shared_expected
    cat <<'CMDS'
cancel_vm_image_download
check_setup_status
check_windows_setup
container_status
download_vm_image
enable_wsl_windows
init_vm
init_wsl
is_vm_running
list_running_containers
reboot_windows
run_initial_setup
vm_image_status
vm_stats
vm_status
CMDS
  } | sort -u
}

linux_expected() {
  {
    shared_expected
    cat <<'CMDS'
cancel_vm_image_download
check_setup_status
check_windows_setup
container_status
download_vm_image
enable_wsl_windows
init_vm
init_wsl
is_vm_running
list_running_containers
reboot_windows
run_initial_setup
vm_image_status
vm_stats
vm_status
CMDS
  } | sort -u
}

compare_platform() {
  local platform="$1"
  local fn_name="$2"

  local actual_file expected_file
  actual_file="$(mktemp)"
  expected_file="$(mktemp)"

  extract_commands "$fn_name" > "$actual_file"

  case "$platform" in
    macos)
      macos_expected > "$expected_file"
      ;;
    windows)
      windows_expected > "$expected_file"
      ;;
    linux)
      linux_expected > "$expected_file"
      ;;
    *)
      echo "[parity] unknown platform: $platform" >&2
      rm -f "$actual_file" "$expected_file"
      exit 1
      ;;
  esac

  echo "[parity] $platform expected=$(wc -l < "$expected_file" | tr -d ' ') actual=$(wc -l < "$actual_file" | tr -d ' ')"

  if ! diff -u "$expected_file" "$actual_file"; then
    echo "[parity] $platform command set mismatch" >&2
    rm -f "$actual_file" "$expected_file"
    return 1
  fi

  rm -f "$actual_file" "$expected_file"
  return 0
}

failed=0

compare_platform "macos" "macos_specta_builder" || failed=1
compare_platform "windows" "run_windows" || failed=1
compare_platform "linux" "run_linux" || failed=1

if [[ "$failed" -ne 0 ]]; then
  echo "[parity] FAILED" >&2
  exit 1
fi

echo "[parity] OK"
