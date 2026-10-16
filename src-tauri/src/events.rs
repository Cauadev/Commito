//! Events pushed from Rust to the WebView: operation progress and repository changes.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const PROGRESS_EVENT: &str = "fg://progress";
pub const REPO_CHANGED_EVENT: &str = "fg://repo-changed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub op_id: String,
    pub repo: Option<String>,
    pub phase: String,
    pub percent: Option<u8>,
    pub line: String,
}

#[derive(Clone)]
pub struct ProgressSink {
    pub app: AppHandle,
    pub op_id: String,
    pub repo: Option<String>,
}

impl ProgressSink {
    pub fn new(app: &AppHandle, op_id: Option<String>, repo: Option<String>) -> Option<Self> {
        op_id.map(|op_id| Self { app: app.clone(), op_id, repo })
    }

    pub fn line(&self, raw: &str) {
        let line = crate::redact::redact(raw);
        let (phase, percent) = parse_progress(&line);
        let _ = self.app.emit(
            PROGRESS_EVENT,
            ProgressPayload { op_id: self.op_id.clone(), repo: self.repo.clone(), phase, percent, line },
        );
    }
}

/// Parses lines such as `Receiving objects:  45% (450/1000), 1.2 MiB | 3 MiB/s`.
pub fn parse_progress(line: &str) -> (String, Option<u8>) {
    let clean = line.trim_start_matches("remote:").trim();
    let phase = clean.split(':').next().unwrap_or("").trim().to_string();
    let percent = clean.find('%').and_then(|idx| {
        let digits: String = clean[..idx].chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let digits: String = digits.chars().rev().collect();
        digits.parse::<u8>().ok()
    });
    (phase, percent)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub repo: String,
    /// `worktree` = files changed; `git` = refs/HEAD/index changed (graph may need reload).
    pub kind: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_progress() {
        let (p, pc) = super::parse_progress("Receiving objects:  45% (450/1000), 1.20 MiB");
        assert_eq!(p, "Receiving objects");
        assert_eq!(pc, Some(45));
        let (p, pc) = super::parse_progress("remote: Counting objects: 100% (3/3), done.");
        assert_eq!(p, "Counting objects");
        assert_eq!(pc, Some(100));
        assert_eq!(super::parse_progress("Cloning into 'x'...").1, None);
    }
}
