//! Cursor IDE integration: warm-handoff workspace launch.
//!
//! `cursor://...prompt?text=` deeplinks cannot target a specific repo: in
//! Cursor 3 the prompt lands in the Agents Window against whatever repo is
//! selected there, which is not the project Opnble just opened. So instead of
//! firing a deeplink, the handoff materializes project-scoped Cursor artifacts
//! (rule file + `.vscode/settings.json`), copies a context-aware prompt to the
//! clipboard, and opens the workspace. The user pastes the prompt into the chat
//! of their choice.

use std::ffi::OsStr;
use std::fmt::Write;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::error::AppError;
use crate::mcp::install as mcp_install;
use crate::projects::marker::{self, Marker};
use crate::projects::types::Project;

const RULE_FILE_REL_PATH: &str = ".cursor/rules/opnble-project.mdc";
const SETTINGS_REL_PATH: &str = ".vscode/settings.json";

const HANDOFF_LABEL_PRESENT: &str = "is running at";
const HANDOFF_LABEL_ABSENT: &str = "is currently not running";

// ==================== Public surface ====================

/// Whether the Cursor binary is reachable on PATH (or installed in a known location).
pub fn cursor_binary_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        is_command_available("cursor") || Path::new("/Applications/Cursor.app").exists()
    }

    #[cfg(target_os = "windows")]
    {
        is_command_available("cursor") || is_command_available("cursor.cmd")
    }

    #[cfg(target_os = "linux")]
    {
        is_command_available("cursor") || is_command_available("cursor-bin")
    }
}

/// Open `project` in Cursor with the warm-handoff sequence.
///
/// Steps:
/// 1. Validate the repo path and Cursor presence (blocking errors).
/// 2. Materialize project-scoped artifacts (best-effort, logs on failure).
/// 3. Copy the handoff prompt to the clipboard before launching, so Opnble
///    still owns focus when the copy happens (best-effort).
/// 4. Launch the Cursor workspace on the repo (blocking error).
///
/// Returns whether the prompt made it to the clipboard, so the caller can tell
/// the user accurately.
pub fn open_in_cursor(
    repo_path: &Path,
    project: &Project,
    port: Option<u16>,
) -> Result<bool, AppError> {
    if !repo_path.exists() {
        return Err(AppError::NotFound {
            entity: "project path".into(),
            id: repo_path.display().to_string(),
        });
    }

    if !cursor_binary_installed() {
        return Err(AppError::Internal {
            reason: "cannot find cursor installation, download it from cursor.com and try again"
                .into(),
        });
    }

    if let Err(e) = materialize_cursor_artifacts(project, port) {
        tracing::warn!(
            "cannot materialize cursor artifacts for {}: {e}",
            project.id
        );
    }

    backfill_project_mcp(repo_path, project);

    let prompt_copied = match copy_to_clipboard(&build_handoff_prompt(project, port)) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("cannot copy cursor handoff prompt for {}: {e}", project.id);
            false
        }
    };

    launch_cursor_workspace(repo_path)?;

    Ok(prompt_copied)
}

/// Build the agent-chat prompt body for the warm handoff.
pub fn build_handoff_prompt(project: &Project, port: Option<u16>) -> String {
    match port {
        Some(p) => format!(
            "This Opnble project '{name}' {label} http://127.0.0.1:{p} on branch {branch}. \
             Open it in the built-in browser and wait for it to load. \
             If you see runtime errors in the console, summarize them and suggest fixes. \
             To make visual edits, switch to the Agents Window (Cmd+Shift+P -> Agents Window) \
             and toggle Design Mode with Cmd+Shift+D.",
            name = project.name,
            label = HANDOFF_LABEL_PRESENT,
            branch = project.branch,
        ),
        None => format!(
            "This Opnble-managed project '{name}' is on branch {branch}. \
             The dev server {label} yet; start it from Opnble before iterating. \
             Once running, ask me to open it in the built-in browser.",
            name = project.name,
            label = HANDOFF_LABEL_ABSENT,
            branch = project.branch,
        ),
    }
}

/// Materialize project-scoped Cursor artifacts inside the cloned repo.
///
/// Writes `.cursor/rules/opnble-project.mdc` (always-applied rule) and merges
/// `.vscode/settings.json` non-destructively. Idempotent: no-ops when the bytes
/// on disk already match the desired content.
pub fn materialize_cursor_artifacts(project: &Project, port: Option<u16>) -> Result<(), AppError> {
    let repo_path = project.repo_path();
    if !repo_path.exists() {
        return Err(AppError::NotFound {
            entity: "project path".into(),
            id: repo_path.display().to_string(),
        });
    }

    write_rule_file(&repo_path, project, port)?;
    write_settings_file(&repo_path, &project.name, port)?;

    Ok(())
}

/// Backfill the MCP wiring Cursor's `opnble` server needs to reach this
/// project: the token-bearing `.cursor/mcp.json`, its `.gitignore` line, and
/// the committed `.opnble` marker. Best-effort, so a project imported before
/// these artifacts existed is healed on the next open without ever blocking
/// the Cursor launch.
fn backfill_project_mcp(repo_path: &Path, project: &Project) {
    let label = project.slug.as_deref().unwrap_or(&project.name);
    mcp_install::install_project_entry(repo_path, label);
    ensure_marker(repo_path, project);
}

/// Write the committed `.opnble` marker when it is absent so cross-machine
/// slug resolution works for projects imported before the marker existed.
/// A present marker is left untouched; a slugless project is skipped since
/// the marker cannot be built without one.
fn ensure_marker(repo_path: &Path, project: &Project) {
    let Some(slug) = project.slug.as_deref() else {
        return;
    };
    match marker::read_marker(repo_path) {
        Ok(Some(_)) => {}
        Ok(None) => {
            let marker = Marker::new(slug.to_string(), project.repo_url.clone());
            if let Err(e) = marker::write_marker(repo_path, &marker) {
                tracing::warn!("cannot write .opnble marker for {slug}: {e}");
            }
        }
        Err(e) => tracing::warn!("cannot read .opnble marker for {slug}: {e}"),
    }
}

// ==================== Workspace launch ====================

/// Open `repo_path` as a Cursor workspace.
///
/// Uses the `cursor` CLI when present, falling back on macOS to `open -a Cursor`
/// which finds `/Applications/Cursor.app` without the CLI on PATH.
fn launch_cursor_workspace(repo_path: &Path) -> Result<(), AppError> {
    let path_arg = repo_path.as_os_str();

    #[cfg(target_os = "macos")]
    {
        if try_launch("cursor", &[path_arg])
            || try_launch("open", &[OsStr::new("-a"), OsStr::new("Cursor"), path_arg])
        {
            return Ok(());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if try_launch("cursor", &[path_arg])
            || try_launch("cursor.cmd", &[path_arg])
            || try_launch(
                "cmd",
                &[
                    OsStr::new("/C"),
                    OsStr::new("start"),
                    OsStr::new(""),
                    OsStr::new("cursor"),
                    path_arg,
                ],
            )
        {
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    {
        if try_launch("cursor", &[path_arg]) || try_launch("cursor-bin", &[path_arg]) {
            return Ok(());
        }
    }

    Err(AppError::Internal {
        reason: "cannot open project in cursor, ensure the `cursor` command works from your shell"
            .into(),
    })
}

// ==================== Clipboard ====================

/// Copy `text` to the OS clipboard via the platform's native CLI.
///
/// macOS `pbcopy`, Windows `clip`, Linux Wayland `wl-copy` then X11 `xclip`.
/// Done before launching Cursor so Opnble still owns focus, and dependency-free
/// to match the rest of this module's shell-out style.
fn copy_to_clipboard(text: &str) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    let candidates: &[(&str, &[&str])] = &[("pbcopy", &[])];
    #[cfg(target_os = "windows")]
    let candidates: &[(&str, &[&str])] = &[("clip", &[])];
    #[cfg(target_os = "linux")]
    let candidates: &[(&str, &[&str])] =
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];

    for (program, args) in candidates {
        if pipe_to_stdin(program, args, text) {
            return Ok(());
        }
    }

    Err(AppError::Internal {
        reason: "cannot copy handoff prompt to clipboard".into(),
    })
}

/// Spawn `program` with `args`, write `input` to its stdin, and report success.
fn pipe_to_stdin(program: &str, args: &[&str], input: &str) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    {
        let Some(mut stdin) = child.stdin.take() else {
            return false;
        };
        if stdin.write_all(input.as_bytes()).is_err() {
            return false;
        }
    }

    child.wait().is_ok_and(|status| status.success())
}

// ==================== Artifact writers ====================

fn write_rule_file(repo_path: &Path, project: &Project, port: Option<u16>) -> Result<(), AppError> {
    let body = build_rule_file_body(project, port);
    let path = repo_path.join(RULE_FILE_REL_PATH);
    write_if_changed(&path, body.as_bytes())
}

fn write_settings_file(
    repo_path: &Path,
    project_name: &str,
    port: Option<u16>,
) -> Result<(), AppError> {
    let path = repo_path.join(SETTINGS_REL_PATH);

    // If the existing file is unparseable, leave it untouched: the user may have
    // hand-edited JSONC/comments and we must not clobber that.
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!(
                    "cannot parse existing {}: {e}; skipping settings merge",
                    path.display()
                );
                return Ok(());
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => return Err(AppError::from(e)),
    };

    // Existing settings.json is somehow a primitive/array; bail rather than
    // overwrite with our shape.
    let Value::Object(mut merged) = existing else {
        tracing::warn!(
            "existing {} is not a JSON object; skipping settings merge",
            path.display()
        );
        return Ok(());
    };

    let desired = build_desired_settings(project_name, port);
    if desired.is_empty() {
        return Ok(());
    }
    merge_json(&mut merged, &desired);

    let mut bytes = serde_json::to_vec_pretty(&Value::Object(merged))?;
    bytes.push(b'\n');
    write_if_changed(&path, &bytes)
}

fn write_if_changed(path: &Path, content: &[u8]) -> Result<(), AppError> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == content {
            return Ok(());
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, content)?;
    Ok(())
}

// ==================== Rule body + settings shape ====================

fn build_rule_file_body(project: &Project, port: Option<u16>) -> String {
    let mut body = String::with_capacity(2048);
    body.push_str("---\nalwaysApply: true\n---\n\n");
    body.push_str("# Opnble project context\n\n");
    body.push_str(
        "This repository is opened by Opnble, a desktop app that runs React/Next.js \
         projects locally inside a managed Podman container. The dev server, dependencies, \
         and port mapping are owned by Opnble: act on them through Opnble's MCP tools, not \
         the host shell.\n\n",
    );

    body.push_str("## Project facts\n\n");
    let _ = writeln!(body, "- Name: {}", project.name);
    let _ = writeln!(body, "- Branch: {}", project.branch);
    match &project.slug {
        Some(slug) => {
            let _ = writeln!(
                body,
                "- Opnble slug (pass as `slug` to every MCP tool): {slug}"
            );
        }
        None => body.push_str(
            "- Opnble slug: not assigned yet. Call `list_projects` and match on the \
             name above to get the slug before calling any other tool.\n",
        ),
    }
    if let Some(p) = port {
        let _ = writeln!(body, "- Dev server URL: http://127.0.0.1:{p}");
    } else {
        body.push_str("- Dev server URL: not running yet (start the project from Opnble)\n");
    }
    body.push_str("- Source tree: mounted into a Podman container managed by Opnble\n\n");

    body.push_str("## Use the Opnble MCP tools (do not improvise)\n\n");
    body.push_str(
        "Opnble runs an MCP server that owns this project. For any project operation, call \
         the named Opnble MCP tool with the slug above. Never substitute a host-shell \
         command, a manual build, or a third-party service.\n\n",
    );
    body.push_str(
        "- Public, shareable, or preview link: use `create_preview_link` (read the live URL \
         with `get_preview_link`, tear it down with `stop_preview_link`). Never deploy to \
         Cloudflare Pages/Workers, Vercel, or Netlify, and never run `cloudflared`, `ngrok`, \
         `vite preview`, or build `dist/` to host it yourself.\n",
    );
    body.push_str(
        "- Start, stop, or restart the dev server: use `start_project`, `stop_project`, \
         `restart_project`. Never run `pnpm dev`/`npm run dev` or kill processes on the host.\n",
    );
    body.push_str(
        "- Inspect runtime errors or output: use `tail_logs`. Do not guess at errors or read \
         host log files.\n",
    );
    body.push_str(
        "- Run a build, test, or any shell command: use `exec_command`. It runs inside the \
         project's container, where Node and the dependencies live; the host has no project \
         toolchain.\n",
    );
    body.push_str(
        "- Git status, branch, or pull: use `git_status`, `switch_branch`, `pull_latest`. \
         Commit and push to GitHub with `github_push`.\n",
    );
    body.push_str(
        "- Publish a static build: use `github_pages_publish` (undo with \
         `github_pages_unpublish`). Never push a `gh-pages` branch by hand.\n",
    );
    body.push_str(
        "- Snapshot, roll back, or fork the working tree: use `snapshot_project`, \
         `rollback_project`, `fork_project`.\n\n",
    );
    body.push_str(
        "If the Opnble MCP server is not connected, say so and ask the user to check Opnble. \
         Do not fall back to a manual deploy or a host-shell workaround.\n\n",
    );

    body.push_str("## Do not\n\n");
    body.push_str(
        "- Do not run `pnpm install` (or `npm install`/`yarn install`) directly. Opnble \
         installs dependencies inside the container.\n",
    );
    body.push_str(
        "- Do not start, stop, or restart the dev server outside Opnble. Use the MCP \
         lifecycle tools above so the container, port allocation, and state stay in sync.\n\n",
    );

    body.push_str("## Visual editing\n\n");
    body.push_str(
        "To make visual edits, open the Agents Window (Cmd+Shift+P -> Agents Window) and \
         toggle Design Mode with Cmd+Shift+D.\n",
    );

    body
}

fn build_desired_settings(project_name: &str, port: Option<u16>) -> Map<String, Value> {
    let mut settings = Map::new();
    if let Some(p) = port {
        let mut port_entry = Map::new();
        port_entry.insert("label".into(), Value::String(project_name.to_string()));
        port_entry.insert("onAutoForward".into(), Value::String("ignore".into()));

        let mut ports_attributes = Map::new();
        ports_attributes.insert(p.to_string(), Value::Object(port_entry));

        settings.insert(
            "remote.portsAttributes".into(),
            Value::Object(ports_attributes),
        );
    }
    settings
}

fn merge_json(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target_inner)), Value::Object(source_inner)) => {
                merge_json(target_inner, source_inner);
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

// ==================== Process helpers ====================

fn is_command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn try_launch(program: &str, args: &[&OsStr]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::projects::types::{Project, ProjectId};

    fn make_project(repo_path: &Path) -> Project {
        Project::new(
            ProjectId::new("test-proj").expect("valid id"),
            "demo-app".to_string(),
            "https://example.com/demo".to_string(),
            repo_path.display().to_string(),
            "feature-branch".to_string(),
        )
    }

    #[test]
    fn handoff_prompt_with_port_includes_url_and_branch() {
        let project = make_project(&PathBuf::from("/tmp/unused"));
        let prompt = build_handoff_prompt(&project, Some(3000));
        assert!(prompt.contains("demo-app"), "name missing: {prompt}");
        assert!(
            prompt.contains("feature-branch"),
            "branch missing: {prompt}"
        );
        assert!(
            prompt.contains("http://127.0.0.1:3000"),
            "url missing: {prompt}"
        );
        assert!(prompt.contains("Cmd+Shift+D"), "design-mode hint missing");
    }

    #[test]
    fn handoff_prompt_without_port_omits_url() {
        let project = make_project(&PathBuf::from("/tmp/unused"));
        let prompt = build_handoff_prompt(&project, None);
        assert!(prompt.contains("demo-app"));
        assert!(prompt.contains("feature-branch"));
        assert!(
            !prompt.contains("http://127.0.0.1") && !prompt.contains("http://localhost"),
            "url leaked: {prompt}"
        );
        assert!(prompt.contains("start it from Opnble"));
    }

    #[test]
    fn rule_body_snapshot_with_port() {
        let base = make_project(&PathBuf::from("/tmp/unused"));
        let project = base.with_slug("demo-app".to_string());
        let body = build_rule_file_body(&project, Some(4173));
        let expected = "---\n\
            alwaysApply: true\n\
            ---\n\
            \n\
            # Opnble project context\n\
            \n\
            This repository is opened by Opnble, a desktop app that runs React/Next.js \
            projects locally inside a managed Podman container. The dev server, dependencies, \
            and port mapping are owned by Opnble: act on them through Opnble's MCP tools, not \
            the host shell.\n\
            \n\
            ## Project facts\n\
            \n\
            - Name: demo-app\n\
            - Branch: feature-branch\n\
            - Opnble slug (pass as `slug` to every MCP tool): demo-app\n\
            - Dev server URL: http://127.0.0.1:4173\n\
            - Source tree: mounted into a Podman container managed by Opnble\n\
            \n\
            ## Use the Opnble MCP tools (do not improvise)\n\
            \n\
            Opnble runs an MCP server that owns this project. For any project operation, call \
            the named Opnble MCP tool with the slug above. Never substitute a host-shell \
            command, a manual build, or a third-party service.\n\
            \n\
            - Public, shareable, or preview link: use `create_preview_link` (read the live URL \
            with `get_preview_link`, tear it down with `stop_preview_link`). Never deploy to \
            Cloudflare Pages/Workers, Vercel, or Netlify, and never run `cloudflared`, `ngrok`, \
            `vite preview`, or build `dist/` to host it yourself.\n\
            - Start, stop, or restart the dev server: use `start_project`, `stop_project`, \
            `restart_project`. Never run `pnpm dev`/`npm run dev` or kill processes on the host.\n\
            - Inspect runtime errors or output: use `tail_logs`. Do not guess at errors or read \
            host log files.\n\
            - Run a build, test, or any shell command: use `exec_command`. It runs inside the \
            project's container, where Node and the dependencies live; the host has no project \
            toolchain.\n\
            - Git status, branch, or pull: use `git_status`, `switch_branch`, `pull_latest`. \
            Commit and push to GitHub with `github_push`.\n\
            - Publish a static build: use `github_pages_publish` (undo with \
            `github_pages_unpublish`). Never push a `gh-pages` branch by hand.\n\
            - Snapshot, roll back, or fork the working tree: use `snapshot_project`, \
            `rollback_project`, `fork_project`.\n\
            \n\
            If the Opnble MCP server is not connected, say so and ask the user to check Opnble. \
            Do not fall back to a manual deploy or a host-shell workaround.\n\
            \n\
            ## Do not\n\
            \n\
            - Do not run `pnpm install` (or `npm install`/`yarn install`) directly. Opnble \
            installs dependencies inside the container.\n\
            - Do not start, stop, or restart the dev server outside Opnble. Use the MCP \
            lifecycle tools above so the container, port allocation, and state stay in sync.\n\
            \n\
            ## Visual editing\n\
            \n\
            To make visual edits, open the Agents Window (Cmd+Shift+P -> Agents Window) and \
            toggle Design Mode with Cmd+Shift+D.\n";
        assert_eq!(body, expected);
    }

    #[test]
    fn rule_body_without_port_says_not_running() {
        let project = make_project(&PathBuf::from("/tmp/unused"));
        let body = build_rule_file_body(&project, None);
        assert!(body.contains("not running yet"));
        assert!(!body.contains("http://127.0.0.1"));
        assert!(!body.contains("http://localhost"));
    }

    #[test]
    fn rule_body_directs_preview_links_to_the_mcp_tool() {
        let base = make_project(&PathBuf::from("/tmp/unused"));
        let project = base.with_slug("demo-app".to_string());
        let body = build_rule_file_body(&project, Some(4173));
        assert!(body.contains("create_preview_link"), "preview tool missing");
        assert!(
            body.contains("Never deploy to Cloudflare"),
            "manual-deploy guardrail missing"
        );
        assert!(
            body.contains("Opnble slug (pass as `slug` to every MCP tool): demo-app"),
            "slug not surfaced for tool calls"
        );
    }

    #[test]
    fn rule_body_without_slug_points_to_list_projects() {
        let project = make_project(&PathBuf::from("/tmp/unused"));
        let body = build_rule_file_body(&project, None);
        assert!(
            body.contains("not assigned yet") && body.contains("list_projects"),
            "slugless project should be told to resolve via list_projects"
        );
    }

    #[test]
    fn ensure_marker_writes_when_absent() {
        let temp = TempDir::new().expect("tempdir");
        let base = make_project(temp.path());
        let project = base.with_slug("demo-app".to_string());

        ensure_marker(temp.path(), &project);

        let written = marker::read_marker(temp.path())
            .expect("read marker")
            .expect("marker should be written when absent");
        assert_eq!(written.slug, "demo-app");
        assert_eq!(written.repo, project.repo_url);
    }

    #[test]
    fn ensure_marker_preserves_existing_marker() {
        let temp = TempDir::new().expect("tempdir");
        let existing = Marker::new(
            "original".to_string(),
            "https://example.com/original".to_string(),
        );
        marker::write_marker(temp.path(), &existing).expect("seed marker");
        let base = make_project(temp.path());
        let project = base.with_slug("demo-app".to_string());

        ensure_marker(temp.path(), &project);

        let loaded = marker::read_marker(temp.path())
            .expect("read marker")
            .expect("marker present");
        assert_eq!(
            loaded.slug, "original",
            "an existing committed marker must not be clobbered"
        );
    }

    #[test]
    fn ensure_marker_skips_when_slug_absent() {
        let temp = TempDir::new().expect("tempdir");
        let project = make_project(temp.path());

        ensure_marker(temp.path(), &project);

        assert!(
            marker::read_marker(temp.path())
                .expect("read marker")
                .is_none(),
            "a slugless project cannot build a marker, so none should be written"
        );
    }

    #[test]
    fn materialize_creates_artifacts_and_is_idempotent() {
        let temp = TempDir::new().expect("tempdir");
        let project = make_project(temp.path());

        materialize_cursor_artifacts(&project, Some(5173)).expect("first call writes");

        let rule_path = temp.path().join(RULE_FILE_REL_PATH);
        let settings_path = temp.path().join(SETTINGS_REL_PATH);
        assert!(rule_path.exists(), "rule file should exist");
        assert!(settings_path.exists(), "settings file should exist");

        let rule_first = std::fs::read(&rule_path).expect("read rule");
        let rule_first_mtime = std::fs::metadata(&rule_path)
            .expect("rule meta")
            .modified()
            .expect("rule mtime");
        let settings_first = std::fs::read(&settings_path).expect("read settings");
        let settings_first_mtime = std::fs::metadata(&settings_path)
            .expect("settings meta")
            .modified()
            .expect("settings mtime");

        // Sleep one filesystem-mtime granularity tick so a real rewrite would be
        // observable. We then assert no rewrite happened.
        std::thread::sleep(std::time::Duration::from_millis(1100));

        materialize_cursor_artifacts(&project, Some(5173)).expect("second call no-ops");

        let rule_second = std::fs::read(&rule_path).expect("read rule again");
        let rule_second_mtime = std::fs::metadata(&rule_path)
            .expect("rule meta again")
            .modified()
            .expect("rule mtime again");
        let settings_second = std::fs::read(&settings_path).expect("read settings again");
        let settings_second_mtime = std::fs::metadata(&settings_path)
            .expect("settings meta again")
            .modified()
            .expect("settings mtime again");

        assert_eq!(rule_first, rule_second, "rule bytes drifted");
        assert_eq!(settings_first, settings_second, "settings bytes drifted");
        assert_eq!(rule_first_mtime, rule_second_mtime, "rule was rewritten");
        assert_eq!(
            settings_first_mtime, settings_second_mtime,
            "settings was rewritten"
        );
    }

    #[test]
    fn materialize_preserves_existing_settings_keys() {
        let temp = TempDir::new().expect("tempdir");
        let project = make_project(temp.path());
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).expect("mkdir .vscode");
        let settings_path = vscode_dir.join("settings.json");

        let existing = json!({
            "editor.fontSize": 14,
            "remote.portsAttributes": {
                "9000": { "label": "user-port", "onAutoForward": "openBrowser" }
            }
        });
        std::fs::write(
            &settings_path,
            serde_json::to_vec_pretty(&existing).expect("serialize"),
        )
        .expect("seed settings");

        materialize_cursor_artifacts(&project, Some(5173)).expect("merge ok");

        let merged_text = std::fs::read_to_string(&settings_path).expect("read merged");
        let merged: Value = serde_json::from_str(&merged_text).expect("parse merged");

        assert_eq!(merged.get("editor.fontSize"), Some(&json!(14)));
        let ports = merged
            .get("remote.portsAttributes")
            .and_then(Value::as_object)
            .expect("portsAttributes object");
        // Existing user port preserved
        assert_eq!(
            ports.get("9000").and_then(|v| v.get("label")),
            Some(&json!("user-port"))
        );
        // Opnble's port added with the project name
        assert_eq!(
            ports.get("5173").and_then(|v| v.get("label")),
            Some(&json!("demo-app"))
        );
        assert_eq!(
            ports.get("5173").and_then(|v| v.get("onAutoForward")),
            Some(&json!("ignore"))
        );
    }

    #[test]
    fn materialize_skips_unparseable_settings_without_clobber() {
        let temp = TempDir::new().expect("tempdir");
        let project = make_project(temp.path());
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).expect("mkdir .vscode");
        let settings_path = vscode_dir.join("settings.json");
        let raw = "// jsonc with comments\n{ \"editor.fontSize\": 14 }\n";
        std::fs::write(&settings_path, raw).expect("seed jsonc");

        materialize_cursor_artifacts(&project, Some(5173)).expect("rule still writes");

        let after = std::fs::read_to_string(&settings_path).expect("read");
        assert_eq!(after, raw, "unparseable settings must not be clobbered");
        assert!(temp.path().join(RULE_FILE_REL_PATH).exists());
    }

    #[cfg(unix)]
    #[test]
    fn copy_to_clipboard_pipes_text_through_a_native_tool() {
        // `cat` stands in for pbcopy/clip: it consumes stdin and exits 0, so a
        // successful pipe proves the stdin plumbing works without touching the
        // real OS clipboard.
        assert!(pipe_to_stdin("cat", &[], "handoff prompt body"));
        assert!(!pipe_to_stdin("definitely-not-a-real-binary-xyz", &[], "x"));
    }
}
