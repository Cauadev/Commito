//! Commit log reading (machine friendly delimiters, never `--graph` ASCII art).

use serde::Serialize;

use super::graph::Edge;

/// `git log -z` format: fields separated by 0x1F, records by NUL.
pub const LOG_FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ct%x1f%s";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub committer_name: String,
    pub committer_time: i64,
    pub subject: String,
}

pub fn parse_log(raw: &[u8]) -> Vec<CommitInfo> {
    raw.split(|b| *b == 0)
        .filter_map(|rec| {
            let rec = String::from_utf8_lossy(rec);
            let rec = rec.trim_start_matches('\n');
            if rec.is_empty() {
                return None;
            }
            let f: Vec<&str> = rec.splitn(8, '\x1f').collect();
            if f.len() < 8 {
                return None;
            }
            Some(CommitInfo {
                hash: f[0].to_string(),
                parents: f[1].split(' ').filter(|p| !p.is_empty()).map(str::to_string).collect(),
                author_name: f[2].to_string(),
                author_email: f[3].to_string(),
                author_time: f[4].parse().unwrap_or(0),
                committer_name: f[5].to_string(),
                committer_time: f[6].parse().unwrap_or(0),
                subject: f[7].to_string(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Commit,
    Wip,
    Stash,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub kind: RowKind,
    pub hash: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub committer_time: i64,
    pub lane: u16,
    pub color: u16,
    pub edges: Vec<Edge>,
    pub stash_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: i64,
    pub subject: String,
    pub body: String,
    pub files: Vec<crate::git::status::FileChange>,
    pub signature: Option<String>,
}

pub const DETAIL_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%G?%x00%B";

pub fn parse_detail(raw: &[u8]) -> Option<CommitDetail> {
    let text = String::from_utf8_lossy(raw);
    let f: Vec<&str> = text.splitn(10, '\0').collect();
    if f.len() < 10 {
        return None;
    }
    let message = f[9].trim_end();
    let (subject, body) = match message.split_once('\n') {
        Some((s, b)) => (s.to_string(), b.trim_start_matches('\n').to_string()),
        None => (message.to_string(), String::new()),
    };
    let sig = match f[8] {
        "G" => Some("good"),
        "B" => Some("bad"),
        "U" => Some("untrusted"),
        "X" | "Y" => Some("expired"),
        "R" => Some("revoked"),
        "E" => Some("unverifiable"),
        _ => None,
    };
    Some(CommitDetail {
        hash: f[0].to_string(),
        parents: f[1].split(' ').filter(|p| !p.is_empty()).map(str::to_string).collect(),
        author_name: f[2].to_string(),
        author_email: f[3].to_string(),
        author_time: f[4].parse().unwrap_or(0),
        committer_name: f[5].to_string(),
        committer_email: f[6].to_string(),
        committer_time: f[7].parse().unwrap_or(0),
        subject,
        body,
        files: Vec::new(),
        signature: sig.map(str::to_string),
    })
}

/// Parses `--name-status -z` output (used for commit file lists).
pub fn parse_name_status(raw: &[u8]) -> Vec<crate::git::status::FileChange> {
    use crate::git::status::{FileChange, FileStatus};
    let fields: Vec<String> = raw.split(|b| *b == 0).map(|f| String::from_utf8_lossy(f).into_owned()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let code = fields[i].trim().to_string();
        i += 1;
        if code.is_empty() {
            continue;
        }
        let status = match code.as_bytes()[0] {
            b'A' => FileStatus::Added,
            b'D' => FileStatus::Deleted,
            b'R' => FileStatus::Renamed,
            b'C' => FileStatus::Copied,
            b'T' => FileStatus::TypeChanged,
            b'U' => FileStatus::Conflicted,
            _ => FileStatus::Modified,
        };
        let (orig, path) = if matches!(status, FileStatus::Renamed | FileStatus::Copied) {
            let o = fields.get(i).cloned();
            let p = fields.get(i + 1).cloned().unwrap_or_default();
            i += 2;
            (o, p)
        } else {
            let p = fields.get(i).cloned().unwrap_or_default();
            i += 1;
            (None, p)
        };
        if path.is_empty() {
            continue;
        }
        out.push(FileChange {
            path,
            orig_path: orig,
            status,
            additions: None,
            deletions: None,
            binary: false,
            submodule: false,
            conflict: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_records() {
        let raw = b"aaa\x1fbbb ccc\x1fAna\x1fana@x.com\x1f100\x1fAna\x1f101\x1fMerge branch 'x'\0ddd\x1f\x1fBo\x1fbo@x\x1f5\x1fBo\x1f6\x1froot: a\x1fb\0";
        let c = parse_log(raw);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].parents, vec!["bbb", "ccc"]);
        assert_eq!(c[0].subject, "Merge branch 'x'");
        assert!(c[1].parents.is_empty());
        assert_eq!(c[1].subject, "root: a\x1fb");
    }

    #[test]
    fn parses_detail_and_name_status() {
        let raw = b"h\0p1 p2\0A\0a@x\x001\0C\0c@x\x002\0N\0Subject line\n\nBody 1\nBody 2\n";
        let d = parse_detail(raw).unwrap();
        assert_eq!(d.subject, "Subject line");
        assert_eq!(d.body, "Body 1\nBody 2");
        assert_eq!(d.parents.len(), 2);
        assert!(d.signature.is_none());

        let ns = parse_name_status(b"M\0src/a.rs\0R100\0old.txt\0new.txt\0A\0n e w.md\0");
        assert_eq!(ns.len(), 3);
        assert_eq!(ns[1].path, "new.txt");
        assert_eq!(ns[1].orig_path.as_deref(), Some("old.txt"));
        assert_eq!(ns[2].path, "n e w.md");
    }
}
