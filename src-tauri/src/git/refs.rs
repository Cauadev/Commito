//! Refs (branches, remote branches, tags), worktrees, stashes and in-progress state parsers.

use serde::Serialize;

pub const REF_FORMAT: &str = "--format=%(refname)%1f%(objectname)%1f%(*objectname)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(HEAD)%1f%(worktreepath)%1f%(objecttype)%1f%(creatordate:unix)%1f%(symref)%1f%(contents:subject)";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Local,
    Remote,
    Tag,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitRef {
    pub name: String,
    pub short: String,
    pub kind: RefKind,
    /// Commit the ref points to (peeled for annotated tags).
    pub target: String,
    pub remote: Option<String>,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub upstream_gone: bool,
    pub is_head: bool,
    pub worktree_path: Option<String>,
    pub annotated: bool,
    pub date: i64,
    pub message: Option<String>,
}

pub fn parse_refs(raw: &str) -> Vec<GitRef> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() < 11 {
            continue;
        }
        let name = f[0];
        if !f[9].is_empty() {
            // symbolic refs such as refs/remotes/origin/HEAD
            continue;
        }
        let (kind, short, remote) = if let Some(s) = name.strip_prefix("refs/heads/") {
            (RefKind::Local, s.to_string(), None)
        } else if let Some(s) = name.strip_prefix("refs/remotes/") {
            let remote = s.split('/').next().map(str::to_string);
            (RefKind::Remote, s.to_string(), remote)
        } else if let Some(s) = name.strip_prefix("refs/tags/") {
            (RefKind::Tag, s.to_string(), None)
        } else {
            continue;
        };
        let annotated = f[7] == "tag";
        let target = if annotated && !f[2].is_empty() { f[2] } else { f[1] }.to_string();
        let (mut ahead, mut behind, mut gone) = (None, None, false);
        let track = f[4].trim();
        if track == "gone" {
            gone = true;
        } else if !f[3].is_empty() {
            ahead = Some(0);
            behind = Some(0);
            for part in track.split(',') {
                let part = part.trim();
                if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.parse().ok();
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.parse().ok();
                }
            }
        }
        out.push(GitRef {
            name: name.to_string(),
            short,
            kind,
            target,
            remote,
            upstream: (!f[3].is_empty()).then(|| f[3].to_string()),
            ahead,
            behind,
            upstream_gone: gone,
            is_head: f[5] == "*",
            worktree_path: (!f[6].is_empty()).then(|| f[6].to_string()),
            annotated,
            date: f[8].parse().unwrap_or(0),
            message: (annotated && !f[10].is_empty()).then(|| f[10].to_string()),
        });
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
    pub lock_reason: Option<String>,
    pub prunable: bool,
    pub prunable_reason: Option<String>,
    pub is_main: bool,
    pub is_current: bool,
    /// None when the status could not be computed (missing folder, bare...).
    pub dirty: Option<bool>,
    pub changed_files: Option<u32>,
}

pub fn parse_worktrees(raw: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    let mut cur: Option<Worktree> = None;
    for line in raw.lines() {
        if line.is_empty() {
            if let Some(w) = cur.take() {
                out.push(w);
            }
            continue;
        }
        let (key, val) = match line.split_once(' ') {
            Some((k, v)) => (k, Some(v.to_string())),
            None => (line, None),
        };
        if key == "worktree" {
            if let Some(w) = cur.take() {
                out.push(w);
            }
            cur = Some(Worktree { path: val.unwrap_or_default(), ..Default::default() });
            continue;
        }
        let Some(w) = cur.as_mut() else { continue };
        match key {
            "HEAD" => w.head = val,
            "branch" => w.branch = val.map(|b| b.trim_start_matches("refs/heads/").to_string()),
            "bare" => w.bare = true,
            "detached" => w.detached = true,
            "locked" => {
                w.locked = true;
                w.lock_reason = val;
            }
            "prunable" => {
                w.prunable = true;
                w.prunable_reason = val;
            }
            _ => {}
        }
    }
    if let Some(w) = cur.take() {
        out.push(w);
    }
    if let Some(first) = out.first_mut() {
        first.is_main = true;
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub index: u32,
    pub reference: String,
    pub hash: String,
    pub base: Option<String>,
    pub message: String,
    pub branch: Option<String>,
    pub time: i64,
    pub has_untracked: bool,
}

pub const STASH_FORMAT: &str = "--format=%gd%x1f%H%x1f%P%x1f%gs%x1f%ct";

pub fn parse_stashes(raw: &[u8]) -> Vec<StashEntry> {
    raw.split(|b| *b == 0)
        .filter_map(|rec| {
            let rec = String::from_utf8_lossy(rec);
            let rec = rec.trim_matches('\n');
            if rec.is_empty() {
                return None;
            }
            let f: Vec<&str> = rec.splitn(5, '\x1f').collect();
            if f.len() < 5 {
                return None;
            }
            let reference = f[0].to_string();
            let index = reference
                .trim_start_matches("stash@{")
                .trim_end_matches('}')
                .parse()
                .unwrap_or(0);
            let parents: Vec<&str> = f[2].split(' ').filter(|p| !p.is_empty()).collect();
            // "WIP on main: abc subject" | "On main: message"
            let gs = f[3];
            let (branch, message) = match gs.split_once(": ") {
                Some((head, msg)) => {
                    let b = head.trim_start_matches("WIP on ").trim_start_matches("On ").to_string();
                    (Some(b), msg.to_string())
                }
                None => (None, gs.to_string()),
            };
            Some(StashEntry {
                index,
                reference,
                hash: f[1].to_string(),
                base: parents.first().map(|p| p.to_string()),
                message,
                branch,
                time: f[4].parse().unwrap_or(0),
                has_untracked: parents.len() >= 3,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    ApplyMailbox,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationState {
    pub kind: OperationKind,
    pub head_name: Option<String>,
    pub onto: Option<String>,
    pub step: Option<u32>,
    pub total: Option<u32>,
    pub current_commit: Option<String>,
    pub message: Option<String>,
}

/// Inspects the (per-worktree) git dir for in-progress operations.
pub fn detect_operation(git_dir: &std::path::Path) -> Option<OperationState> {
    let read = |p: &str| std::fs::read_to_string(git_dir.join(p)).ok().map(|s| s.trim().to_string());
    let msg = read("MERGE_MSG");
    if git_dir.join("rebase-merge").is_dir() {
        return Some(OperationState {
            kind: OperationKind::Rebase,
            head_name: read("rebase-merge/head-name").map(|h| h.trim_start_matches("refs/heads/").to_string()),
            onto: read("rebase-merge/onto"),
            step: read("rebase-merge/msgnum").and_then(|s| s.parse().ok()),
            total: read("rebase-merge/end").and_then(|s| s.parse().ok()),
            current_commit: read("rebase-merge/stopped-sha"),
            message: msg,
        });
    }
    if git_dir.join("rebase-apply").is_dir() {
        let applying = git_dir.join("rebase-apply/applying").exists();
        return Some(OperationState {
            kind: if applying { OperationKind::ApplyMailbox } else { OperationKind::Rebase },
            head_name: read("rebase-apply/head-name").map(|h| h.trim_start_matches("refs/heads/").to_string()),
            onto: read("rebase-apply/onto"),
            step: read("rebase-apply/next").and_then(|s| s.parse().ok()),
            total: read("rebase-apply/last").and_then(|s| s.parse().ok()),
            current_commit: None,
            message: msg,
        });
    }
    let simple = |kind, head: &str| {
        Some(OperationState {
            kind,
            head_name: None,
            onto: None,
            step: None,
            total: None,
            current_commit: read(head),
            message: msg.clone(),
        })
    };
    if git_dir.join("MERGE_HEAD").exists() {
        return simple(OperationKind::Merge, "MERGE_HEAD");
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() || git_dir.join("sequencer/todo").exists() && read("sequencer/todo").is_some_and(|t| t.starts_with("pick")) {
        return simple(OperationKind::CherryPick, "CHERRY_PICK_HEAD");
    }
    if git_dir.join("REVERT_HEAD").exists() {
        return simple(OperationKind::Revert, "REVERT_HEAD");
    }
    if git_dir.join("BISECT_LOG").exists() {
        return simple(OperationKind::Bisect, "BISECT_EXPECTED_REV");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_refs() {
        let raw = "refs/heads/main\x1faaa\x1f\x1forigin/main\x1fahead 2, behind 1\x1f*\x1fC:/r\x1fcommit\x1f10\x1f\x1fsubj\n\
refs/heads/gone\x1fbbb\x1f\x1forigin/gone\x1fgone\x1f \x1f\x1fcommit\x1f11\x1f\x1fs\n\
refs/remotes/origin/HEAD\x1faaa\x1f\x1f\x1f\x1f \x1f\x1fcommit\x1f1\x1frefs/remotes/origin/main\x1fs\n\
refs/remotes/origin/feature/x\x1fccc\x1f\x1f\x1f\x1f \x1f\x1fcommit\x1f12\x1f\x1fs\n\
refs/tags/v1\x1ftagobj\x1fpeeled\x1f\x1f\x1f \x1f\x1ftag\x1f13\x1f\x1fRelease 1\n";
        let r = parse_refs(raw);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].short, "main");
        assert!(r[0].is_head);
        assert_eq!((r[0].ahead, r[0].behind), (Some(2), Some(1)));
        assert_eq!(r[0].worktree_path.as_deref(), Some("C:/r"));
        assert!(r[1].upstream_gone);
        assert_eq!(r[2].kind, RefKind::Remote);
        assert_eq!(r[2].remote.as_deref(), Some("origin"));
        assert_eq!(r[2].short, "origin/feature/x");
        assert_eq!(r[3].target, "peeled");
        assert!(r[3].annotated);
        assert_eq!(r[3].message.as_deref(), Some("Release 1"));
    }

    #[test]
    fn parses_worktrees() {
        let raw = "worktree C:/repo\nHEAD aaa\nbranch refs/heads/main\n\nworktree C:/wt/feat\nHEAD bbb\nbranch refs/heads/feat\nlocked on usb drive\n\nworktree C:/wt/det\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n\n";
        let w = parse_worktrees(raw);
        assert_eq!(w.len(), 3);
        assert!(w[0].is_main);
        assert_eq!(w[1].branch.as_deref(), Some("feat"));
        assert!(w[1].locked);
        assert_eq!(w[1].lock_reason.as_deref(), Some("on usb drive"));
        assert!(w[2].detached && w[2].prunable);
    }

    #[test]
    fn parses_stashes() {
        let raw = b"stash@{0}\x1fs1\x1fbase idx untr\x1fOn main: my work\x1f50\0stash@{1}\x1fs2\x1fb2 i2\x1fWIP on feat: abc fix\x1f40\0";
        let s = parse_stashes(raw);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].message, "my work");
        assert_eq!(s[0].branch.as_deref(), Some("main"));
        assert!(s[0].has_untracked);
        assert_eq!(s[1].index, 1);
        assert_eq!(s[1].base.as_deref(), Some("b2"));
    }
}
