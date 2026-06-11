//! Host-side file watcher for hot reload.
//!
//! Watches project directories on the host. When a file changes,
//! sends a `TouchFile` RPC to the guest agent to trigger inotify
//! inside the VM (since `VirtioFS` doesn't propagate inotify events).

use std::collections::HashMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant};

use super::agent_client::AgentClient;
use super::VmError;

/// Cooldown after sending a touch RPC before processing new events for the same path.
///
/// Prevents a feedback loop: touch updates mtime on the `VirtioFS` mount, which
/// propagates back to the host and triggers another file watcher event, which
/// would send another touch, ad infinitum.
const TOUCH_COOLDOWN: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
enum FsAction {
    Touch { path: String },
    EnsurePath { path: String, is_dir: bool },
    RemovePath { path: String, recursive: bool },
    RenamePath { old_path: String, new_path: String },
}

impl fmt::Display for FsAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Touch { path } => write!(f, "Touch {path}"),
            Self::EnsurePath { path, .. } => write!(f, "EnsurePath {path}"),
            Self::RemovePath { path, .. } => write!(f, "RemovePath {path}"),
            Self::RenamePath { old_path, new_path } => {
                write!(f, "RenamePath {old_path} -> {new_path}")
            }
        }
    }
}

fn is_ignored_path(relative: &Path) -> bool {
    relative.components().any(|component| {
        let Component::Normal(seg) = component else {
            return false;
        };
        matches!(
            seg.to_str(),
            Some("node_modules" | ".git" | ".next" | "target")
        )
    })
}

fn relative_to_guest_suffix(relative: &Path) -> Option<String> {
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(seg) => segments.push(seg.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    Some(segments.join("/"))
}

fn to_guest_path(host_base: &Path, guest_base: &str, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(host_base).ok()?;
    if is_ignored_path(relative) {
        return None;
    }
    let suffix = relative_to_guest_suffix(relative)?;
    if suffix.is_empty() {
        Some(guest_base.to_string())
    } else {
        Some(format!("{guest_base}/{suffix}"))
    }
}

fn path_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

fn collect_guest_tree_snapshot(host_base: &Path, guest_base: &str) -> HashMap<String, bool> {
    let mut snapshot = HashMap::new();
    snapshot.insert(guest_base.to_string(), true);

    if !host_base.exists() || !host_base.is_dir() {
        return snapshot;
    }

    let mut stack = vec![host_base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            let Ok(relative) = path.strip_prefix(host_base) else {
                continue;
            };
            if is_ignored_path(relative) {
                continue;
            }

            let Some(guest_path) = to_guest_path(host_base, guest_base, &path) else {
                continue;
            };

            let is_dir = file_type.is_dir();
            snapshot.insert(guest_path, is_dir);
            if is_dir {
                stack.push(path);
            }
        }
    }

    snapshot
}

async fn collect_guest_tree_snapshot_async(
    host_base: PathBuf,
    guest_base: String,
) -> HashMap<String, bool> {
    let fallback_guest_base = guest_base.clone();
    match tokio::task::spawn_blocking(move || collect_guest_tree_snapshot(&host_base, &guest_base))
        .await
    {
        Ok(snapshot) => snapshot,
        Err(e) => {
            tracing::error!(
                "Snapshot collection task failed; falling back to root-only snapshot: {}",
                e
            );
            let mut snapshot = HashMap::new();
            snapshot.insert(fallback_guest_base, true);
            snapshot
        }
    }
}

fn apply_action_to_known_paths(known_paths: &mut HashMap<String, bool>, action: &FsAction) {
    match action {
        FsAction::Touch { .. } => {}
        FsAction::EnsurePath { path, is_dir } => {
            known_paths.insert(path.clone(), *is_dir);
        }
        FsAction::RemovePath { path, recursive } => {
            known_paths.remove(path);
            if *recursive {
                let prefix = format!("{}/", path.trim_end_matches('/'));
                known_paths.retain(|existing, _| !existing.starts_with(&prefix));
            }
        }
        FsAction::RenamePath { old_path, new_path } => {
            let moved = known_paths.remove(old_path);
            if let Some(is_dir) = moved {
                known_paths.insert(new_path.clone(), is_dir);
            } else {
                known_paths.insert(new_path.clone(), false);
            }

            let old_prefix = format!("{}/", old_path.trim_end_matches('/'));
            let new_prefix = format!("{}/", new_path.trim_end_matches('/'));
            let children: Vec<(String, bool)> = known_paths
                .iter()
                .filter_map(|(path, is_dir)| {
                    if path.starts_with(&old_prefix) {
                        Some((path.clone(), *is_dir))
                    } else {
                        None
                    }
                })
                .collect();

            for (child_path, is_dir) in children {
                known_paths.remove(&child_path);
                let suffix = &child_path[old_prefix.len()..];
                known_paths.insert(format!("{new_prefix}{suffix}"), is_dir);
            }
        }
    }
}

async fn reconcile_dropped_events(
    agent: &Arc<AgentClient>,
    host_base: &Path,
    guest_base: &str,
    known_paths: &mut HashMap<String, bool>,
    fs_ops_v2_available: bool,
    dropped_events: u64,
) {
    if dropped_events == 0 {
        return;
    }

    tracing::warn!("Reconciling after dropping {dropped_events} file event(s)");

    let current_paths =
        collect_guest_tree_snapshot_async(host_base.to_path_buf(), guest_base.to_string()).await;

    if fs_ops_v2_available {
        // Remove stale paths deepest-first so children are removed before parents.
        let mut stale_paths: Vec<(String, bool)> = known_paths
            .iter()
            .filter_map(|(path, is_dir)| {
                if !current_paths.contains_key(path) && path != guest_base {
                    Some((path.clone(), *is_dir))
                } else {
                    None
                }
            })
            .collect();
        stale_paths.sort_by_key(|p| std::cmp::Reverse(path_depth(&p.0)));
        for (path, is_dir) in stale_paths {
            if let Err(e) = agent.remove_path(&path, is_dir).await {
                tracing::error!("Reconcile remove failed ({path}): {e}");
            }
        }

        // Ensure missing/changed paths shallowest-first so parents exist first.
        let mut ensure_paths: Vec<(String, bool)> = current_paths
            .iter()
            .filter_map(|(path, is_dir)| match known_paths.get(path) {
                Some(prev) if prev == is_dir => None,
                _ => Some((path.clone(), *is_dir)),
            })
            .collect();
        ensure_paths.sort_by_key(|a| path_depth(&a.0));
        for (path, is_dir) in ensure_paths {
            if let Err(e) = agent.ensure_path(&path, is_dir).await {
                tracing::error!("Reconcile ensure failed ({path}): {e}");
            }
        }
    } else if let Err(e) = agent.touch_file(guest_base).await {
        tracing::error!("Reconcile touch fallback failed ({guest_base}): {e}");
    }

    *known_paths = current_paths;
}

fn action_for_create(kind: CreateKind, guest_path: String, fs_ops_v2: bool) -> FsAction {
    if !fs_ops_v2 {
        return FsAction::Touch { path: guest_path };
    }
    let is_dir = matches!(kind, CreateKind::Folder);
    FsAction::EnsurePath {
        path: guest_path,
        is_dir,
    }
}

fn action_for_modify(kind: ModifyKind, guest_path: String) -> FsAction {
    let _ = kind;
    FsAction::Touch { path: guest_path }
}

fn action_for_remove(kind: RemoveKind, guest_path: String, fs_ops_v2: bool) -> FsAction {
    if !fs_ops_v2 {
        return FsAction::Touch { path: guest_path };
    }
    let recursive = matches!(kind, RemoveKind::Folder | RemoveKind::Any);
    FsAction::RemovePath {
        path: guest_path,
        recursive,
    }
}

fn actions_from_event(
    host_base: &Path,
    guest_base: &str,
    event: &Event,
    fs_ops_v2: bool,
) -> Vec<FsAction> {
    let mut actions = Vec::new();
    match &event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() >= 2 => {
            if let (Some(old_host), Some(new_host)) = (event.paths.first(), event.paths.get(1)) {
                if let (Some(old_path), Some(new_path)) = (
                    to_guest_path(host_base, guest_base, old_host),
                    to_guest_path(host_base, guest_base, new_host),
                ) {
                    if fs_ops_v2 {
                        actions.push(FsAction::RenamePath { old_path, new_path });
                    } else {
                        actions.push(FsAction::Touch { path: new_path });
                    }
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 1 => {
            if let Some(guest_path) = event
                .paths
                .first()
                .and_then(|p| to_guest_path(host_base, guest_base, p))
            {
                let parent = Path::new(&guest_path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .map(String::from)
                    .unwrap_or(guest_path);
                actions.push(FsAction::Touch { path: parent });
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            for path in &event.paths {
                if let Some(guest_path) = to_guest_path(host_base, guest_base, path) {
                    if fs_ops_v2 {
                        actions.push(FsAction::RemovePath {
                            path: guest_path,
                            // Treat as recursive to safely handle renamed directories.
                            recursive: true,
                        });
                    } else {
                        actions.push(FsAction::Touch { path: guest_path });
                    }
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in &event.paths {
                if let Some(guest_path) = to_guest_path(host_base, guest_base, path) {
                    if fs_ops_v2 {
                        let is_dir = std::fs::metadata(path).is_ok_and(|m| m.is_dir());
                        actions.push(FsAction::EnsurePath {
                            path: guest_path,
                            is_dir,
                        });
                    } else {
                        actions.push(FsAction::Touch { path: guest_path });
                    }
                }
            }
        }
        EventKind::Create(kind) => {
            for path in &event.paths {
                if let Some(guest_path) = to_guest_path(host_base, guest_base, path) {
                    actions.push(action_for_create(*kind, guest_path, fs_ops_v2));
                }
            }
        }
        EventKind::Modify(kind) => {
            for path in &event.paths {
                if let Some(guest_path) = to_guest_path(host_base, guest_base, path) {
                    actions.push(action_for_modify(*kind, guest_path));
                }
            }
        }
        EventKind::Remove(kind) => {
            for path in &event.paths {
                if let Some(guest_path) = to_guest_path(host_base, guest_base, path) {
                    actions.push(action_for_remove(*kind, guest_path, fs_ops_v2));
                }
            }
        }
        EventKind::Any | EventKind::Access(_) | EventKind::Other => {}
    }
    actions
}

async fn apply_action(agent: &Arc<AgentClient>, action: &FsAction) {
    let result = match action {
        FsAction::Touch { path } => agent.touch_file(path).await,
        FsAction::EnsurePath { path, is_dir } => agent.ensure_path(path, *is_dir).await,
        FsAction::RemovePath { path, recursive } => agent.remove_path(path, *recursive).await,
        FsAction::RenamePath { old_path, new_path } => agent.rename_path(old_path, new_path).await,
    };
    if let Err(e) = result {
        tracing::error!("Apply action failed ({action}): {e}");
    }
}

/// Watches a directory and notifies the guest agent of changes.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    shutdown_tx: mpsc::Sender<()>,
}

impl FileWatcher {
    /// Start watching a project directory for changes.
    ///
    /// When a file is modified on the host, sends a `TouchFile` RPC
    /// to the guest agent to trigger inotify inside the VM.
    pub fn start(
        project_path: &Path,
        guest_base_path: String,
        agent: Arc<AgentClient>,
    ) -> Result<Self, VmError> {
        let fs_ops_v2_enabled = std::env::var("OPNBLE_FS_OPS_V2").is_ok_and(|v| v != "0");
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (notify_tx, mut notify_rx) = mpsc::channel::<Event>(100);
        let dropped_count = Arc::new(AtomicU64::new(0));

        let notify_tx_clone = notify_tx.clone();
        let dropped_for_callback = Arc::clone(&dropped_count);
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if notify_tx_clone.try_send(event).is_err() {
                        let n = dropped_for_callback.fetch_add(1, Ordering::Relaxed) + 1;
                        if n == 1 || n.is_multiple_of(100) {
                            tracing::warn!("Dropped {n} file event(s) (channel backpressure)");
                        }
                    }
                }
            },
            Config::default(),
        )
        .map_err(|e| VmError::FileWatchFailed {
            reason: format!("cannot create file watcher: {e}"),
        })?;

        watcher
            .watch(project_path, RecursiveMode::Recursive)
            .map_err(|e| VmError::FileWatchFailed {
                reason: format!("cannot watch directory: {e}"),
            })?;

        let host_base = project_path.to_path_buf();
        let cap_refresh_interval = Duration::from_secs(30);
        let reconcile_interval = Duration::from_secs(2);
        tokio::spawn(async move {
            let mut fs_ops_v2_available = if fs_ops_v2_enabled {
                agent.fs_ops_v2_supported().await.unwrap_or(false)
            } else {
                false
            };
            let mut known_paths =
                collect_guest_tree_snapshot_async(host_base.clone(), guest_base_path.clone()).await;
            // Tracks paths recently touched via RPC to suppress feedback events.
            // VirtioFS shares the host filesystem, so a touch in the VM changes
            // the host file's mtime, which triggers another watcher event.
            let mut touch_cooldowns: HashMap<String, Instant> = HashMap::new();
            let mut cap_refresh = interval(cap_refresh_interval);
            cap_refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut reconcile_tick = interval(reconcile_interval);
            reconcile_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    Some(event) = notify_rx.recv() => {
                        let actions = actions_from_event(
                            &host_base,
                            &guest_base_path,
                            &event,
                            fs_ops_v2_available,
                        );
                        let now = Instant::now();
                        for action in actions {
                            if let FsAction::Touch { ref path } = action {
                                if let Some(&last_touched) = touch_cooldowns.get(path) {
                                    if now.duration_since(last_touched) < TOUCH_COOLDOWN {
                                        continue;
                                    }
                                }
                            }
                            apply_action(&agent, &action).await;
                            apply_action_to_known_paths(&mut known_paths, &action);
                            if let FsAction::Touch { ref path } = action {
                                touch_cooldowns.insert(path.clone(), Instant::now());
                            }
                        }
                    }
                    _ = cap_refresh.tick() => {
                        if fs_ops_v2_enabled {
                            if let Ok(v) = agent.fs_ops_v2_supported().await {
                                fs_ops_v2_available = v;
                            }
                        }
                    }
                    _ = reconcile_tick.tick() => {
                        let now = Instant::now();
                        touch_cooldowns.retain(|_, last_touched| {
                            now.duration_since(*last_touched) < TOUCH_COOLDOWN
                        });
                        let dropped = dropped_count.swap(0, Ordering::Relaxed);
                        reconcile_dropped_events(
                            &agent,
                            &host_base,
                            &guest_base_path,
                            &mut known_paths,
                            fs_ops_v2_available,
                            dropped,
                        ).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            shutdown_tx,
        })
    }

    /// Stop watching.
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventAttributes};
    use tempfile::TempDir;

    #[test]
    fn maps_rename_event_to_rename_action_when_v2_enabled() {
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![
                PathBuf::from("/tmp/repo/old.txt"),
                PathBuf::from("/tmp/repo/new.txt"),
            ],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::RenamePath {
                old_path: "/repos/p1/old.txt".to_string(),
                new_path: "/repos/p1/new.txt".to_string(),
            }]
        );
    }

    #[test]
    fn maps_create_folder_to_ensure_dir_when_v2_enabled() {
        let event = Event {
            kind: EventKind::Create(CreateKind::Folder),
            paths: vec![PathBuf::from("/tmp/repo/src")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::EnsurePath {
                path: "/repos/p1/src".to_string(),
                is_dir: true,
            }]
        );
    }

    #[test]
    fn maps_remove_to_touch_when_v2_disabled() {
        let event = Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![PathBuf::from("/tmp/repo/a.txt")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, false);
        assert_eq!(
            actions,
            vec![FsAction::Touch {
                path: "/repos/p1/a.txt".to_string()
            }]
        );
    }

    #[test]
    fn ignores_noise_paths_by_segment() {
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/tmp/repo/src/node_modules/a.js")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert!(actions.is_empty());
    }

    #[test]
    fn maps_create_file_to_touch_when_v2_disabled() {
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/tmp/repo/src/a.ts")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, false);
        assert_eq!(
            actions,
            vec![FsAction::Touch {
                path: "/repos/p1/src/a.ts".to_string(),
            }]
        );
    }

    #[test]
    fn maps_create_file_to_ensure_path_when_v2_enabled() {
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/tmp/repo/src/a.ts")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::EnsurePath {
                path: "/repos/p1/src/a.ts".to_string(),
                is_dir: false,
            }]
        );
    }

    #[test]
    fn maps_remove_file_to_remove_path_when_v2_enabled() {
        let event = Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![PathBuf::from("/tmp/repo/a.txt")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::RemovePath {
                path: "/repos/p1/a.txt".to_string(),
                recursive: false,
            }]
        );
    }

    #[test]
    fn maps_modify_to_touch_regardless_of_v2() {
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Any)),
            paths: vec![PathBuf::from("/tmp/repo/src/foo.ts")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::Touch {
                path: "/repos/p1/src/foo.ts".to_string(),
            }]
        );
    }

    #[test]
    fn rename_single_path_falls_back_to_touch_parent() {
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![PathBuf::from("/tmp/repo/old.txt")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::Touch {
                path: "/repos/p1".to_string(),
            }]
        );
    }

    #[test]
    fn rename_from_maps_to_remove_path_when_v2_enabled() {
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            paths: vec![PathBuf::from("/tmp/repo/old.txt")],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(Path::new("/tmp/repo"), "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::RemovePath {
                path: "/repos/p1/old.txt".to_string(),
                recursive: true,
            }]
        );
    }

    #[test]
    fn rename_to_maps_to_ensure_path_when_v2_enabled() {
        let dir = TempDir::new().unwrap();
        let host_base = dir.path().join("repo");
        std::fs::create_dir_all(&host_base).unwrap();
        let new_path = host_base.join("new.txt");
        std::fs::write(&new_path, "hello").unwrap();

        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths: vec![new_path],
            attrs: EventAttributes::default(),
        };
        let actions = actions_from_event(&host_base, "/repos/p1", &event, true);
        assert_eq!(
            actions,
            vec![FsAction::EnsurePath {
                path: "/repos/p1/new.txt".to_string(),
                is_dir: false,
            }]
        );
    }

    #[test]
    fn to_guest_path_maps_repo_root_without_dot_suffix() {
        let path = Path::new("/tmp/repo");
        let guest = to_guest_path(Path::new("/tmp/repo"), "/repos/p1", path);
        assert_eq!(guest.as_deref(), Some("/repos/p1"));
    }

    #[test]
    fn apply_action_to_known_paths_updates_rename_children() {
        let mut known = HashMap::new();
        known.insert("/repos/p1/src".to_string(), true);
        known.insert("/repos/p1/src/a.ts".to_string(), false);
        known.insert("/repos/p1/src/nested".to_string(), true);
        known.insert("/repos/p1/src/nested/b.ts".to_string(), false);

        apply_action_to_known_paths(
            &mut known,
            &FsAction::RenamePath {
                old_path: "/repos/p1/src".to_string(),
                new_path: "/repos/p1/app".to_string(),
            },
        );

        assert!(!known.contains_key("/repos/p1/src"));
        assert!(!known.contains_key("/repos/p1/src/a.ts"));
        assert!(known.contains_key("/repos/p1/app"));
        assert!(known.contains_key("/repos/p1/app/a.ts"));
        assert!(known.contains_key("/repos/p1/app/nested/b.ts"));
    }

    #[test]
    fn display_touch() {
        let action = FsAction::Touch {
            path: "/repos/p1/file.ts".to_string(),
        };
        assert_eq!(format!("{action}"), "Touch /repos/p1/file.ts");
    }

    #[test]
    fn display_ensure_path() {
        let action = FsAction::EnsurePath {
            path: "/repos/p1/src".to_string(),
            is_dir: true,
        };
        assert_eq!(format!("{action}"), "EnsurePath /repos/p1/src");
    }

    #[test]
    fn display_remove_path() {
        let action = FsAction::RemovePath {
            path: "/repos/p1/old.ts".to_string(),
            recursive: true,
        };
        assert_eq!(format!("{action}"), "RemovePath /repos/p1/old.ts");
    }

    #[test]
    fn display_rename_path() {
        let action = FsAction::RenamePath {
            old_path: "/repos/p1/a.ts".to_string(),
            new_path: "/repos/p1/b.ts".to_string(),
        };
        assert_eq!(
            format!("{action}"),
            "RenamePath /repos/p1/a.ts -> /repos/p1/b.ts"
        );
    }

    #[test]
    fn collect_guest_tree_snapshot_includes_root() {
        let dir = TempDir::new().unwrap();
        let host_base = dir.path().join("repo");
        std::fs::create_dir_all(host_base.join("src")).unwrap();
        std::fs::write(host_base.join("src/main.ts"), "x").unwrap();

        let snapshot = collect_guest_tree_snapshot(&host_base, "/repos/p1");
        assert_eq!(snapshot.get("/repos/p1"), Some(&true));
        assert_eq!(snapshot.get("/repos/p1/src"), Some(&true));
        assert_eq!(snapshot.get("/repos/p1/src/main.ts"), Some(&false));
    }
}
