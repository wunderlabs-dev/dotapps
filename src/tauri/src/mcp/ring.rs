//! In-memory ring buffer of recent MCP tool invocations.
//!
//! The Cursor-integration settings panel renders the most recent 50 entries
//! so the user has visibility into what host-side agents have been doing.
//! Records are append-only at the tail; eviction is FIFO from the head once
//! the cap is reached. The buffer lives only for the lifetime of the app
//! process; it intentionally does not persist (the goal is "what just
//! happened," not an audit log).

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

const DEFAULT_CAP: usize = 50;

/// One recorded MCP tool invocation. Wire-compatible with the frontend via
/// `specta::Type` + `Serialize`.
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RingEntry {
    /// Unix millis when the invocation completed.
    pub ts: u64,
    /// Tool name (e.g. "`read_file`"). The MCP tool name on the wire, not the Rust fn name.
    pub tool: String,
    /// Project slug the call targeted, or empty if not slug-scoped.
    pub slug: String,
    /// "ok" on success, "err:CODE" on failure. CODE is the data.code from `McpError`.
    pub outcome: String,
}

/// Bounded ring of recent entries. Cap is fixed at construction.
pub struct ToolRing {
    cap: usize,
    inner: Arc<RwLock<VecDeque<RingEntry>>>,
}

impl ToolRing {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(cap))),
        }
    }

    /// Convenience constructor with the default cap (50).
    pub fn with_default_cap() -> Self {
        Self::new(DEFAULT_CAP)
    }

    /// Record an invocation. `outcome` is "ok" or "err:CODE". `slug` may be
    /// empty for tools that aren't slug-scoped. ts is current unix millis.
    pub fn record(
        &self,
        tool: impl Into<String>,
        slug: impl Into<String>,
        outcome: impl Into<String>,
    ) {
        let entry = RingEntry {
            ts: crate::projects::types::unix_now_millis(),
            tool: tool.into(),
            slug: slug.into(),
            outcome: outcome.into(),
        };
        let mut guard = match self.inner.write() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!("tool ring poisoned, recovering: {e}");
                e.into_inner()
            }
        };
        guard.push_back(entry);
        while guard.len() > self.cap {
            guard.pop_front();
        }
    }

    /// Snapshot the current ring contents as a Vec (oldest first).
    pub fn snapshot(&self) -> Vec<RingEntry> {
        match self.inner.read() {
            Ok(g) => g.iter().cloned().collect(),
            Err(e) => e.into_inner().iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_caps_at_default_and_evicts_oldest_first() {
        let ring = ToolRing::new(3);
        ring.record("read_file", "blog", "ok");
        ring.record("read_file", "blog", "ok");
        ring.record("read_file", "blog", "ok");
        ring.record("read_file", "blog", "err:PROJECT_NOT_FOUND");
        let entries = ring.snapshot();
        assert_eq!(entries.len(), 3);
        // The oldest (first inserted) was evicted.
        assert_eq!(entries.first().unwrap().outcome, "ok");
        assert_eq!(entries.get(2).unwrap().outcome, "err:PROJECT_NOT_FOUND");
    }

    #[test]
    fn ring_default_cap_is_fifty() {
        let ring = ToolRing::with_default_cap();
        for i in 0..60 {
            ring.record("read_file", "blog", format!("ok-{i}"));
        }
        let entries = ring.snapshot();
        assert_eq!(entries.len(), 50);
        assert_eq!(entries.first().unwrap().outcome, "ok-10");
        assert_eq!(entries.get(49).unwrap().outcome, "ok-59");
    }

    #[test]
    fn ring_records_outcome_verbatim() {
        let ring = ToolRing::new(10);
        ring.record("exec_command", "blog", "err:EXEC_TIMEOUT");
        let entries = ring.snapshot();
        let first = entries.first().unwrap();
        assert_eq!(first.tool, "exec_command");
        assert_eq!(first.slug, "blog");
        assert_eq!(first.outcome, "err:EXEC_TIMEOUT");
    }

    #[test]
    fn ring_entry_serializes_camel_case() {
        let entry = RingEntry {
            ts: 1_700_000_000,
            tool: "read_file".into(),
            slug: "blog".into(),
            outcome: "ok".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"ts\":1700000000"));
        assert!(json.contains("\"tool\":\"read_file\""));
        assert!(json.contains("\"slug\":\"blog\""));
        assert!(json.contains("\"outcome\":\"ok\""));
    }
}
