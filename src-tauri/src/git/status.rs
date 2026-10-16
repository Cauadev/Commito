//! Parser for `git status --porcelain=v2 -z --branch --show-stash`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Ignored,
    Conflicted,
}

impl FileStatus {
    fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            b'M' => Self::Modified,
            b'A' => Self::Added,
            b'D' => Self::Deleted,
            b'R' => Self::Renamed,
            b'C' => Self::Copied,
            b'T' => Self::TypeChanged,
            b'U' => Self::Conflicted,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub orig_path: Option<String>,
    pub status: FileStatus,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub binary: bool,
    pub submodule: bool,
    /// Raw two letter XY code for conflicted entries (`UU`, `AA`, `DU`...).
    pub conflict: Option<String>,
}

impl FileChange {
    fn new(path: String, status: FileStatus) -> Self {
        Self { path, orig_path: None, status, additions: None, deletions: None, binary: false, submodule: false, conflict: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    pub oid: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub unborn: bool,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedStatus {
    pub head: HeadInfo,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub conflicted: Vec<FileChange>,
    pub ignored: Vec<String>,
    pub stash_count: u32,
}

pub fn parse_status(raw: &[u8]) -> ParsedStatus {
    let mut st = ParsedStatus::default();
    let fields: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
    let mut i = 0;
    while i < fields.len() {
        let rec = fields[i];
        i += 1;
        if rec.is_empty() {
            continue;
        }
        let text = String::from_utf8_lossy(rec);
        match rec[0] {
            b'#' => parse_header(&text, &mut st),
            b'1' => {
                // 1 XY sub mH mI mW hH hI path
                let parts: Vec<&str> = text.splitn(9, ' ').collect();
                if parts.len() < 9 {
                    continue;
                }
                push_ordinary(&mut st, parts[1].as_bytes(), parts[2], parts[8].to_string(), None);
            }
            b'2' => {
                // 2 XY sub mH mI mW hH hI Xscore path \0 origPath
                let parts: Vec<&str> = text.splitn(10, ' ').collect();
                let orig = fields.get(i).map(|f| String::from_utf8_lossy(f).into_owned());
                i += 1;
                if parts.len() < 10 {
                    continue;
                }
                push_ordinary(&mut st, parts[1].as_bytes(), parts[2], parts[9].to_string(), orig);
            }
            b'u' => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let parts: Vec<&str> = text.splitn(11, ' ').collect();
                if parts.len() < 11 {
                    continue;
                }
                let mut fc = FileChange::new(parts[10].to_string(), FileStatus::Conflicted);
                fc.conflict = Some(parts[1].to_string());
                fc.submodule = parts[2].starts_with('S');
                st.conflicted.push(fc);
            }
            b'?' => st.unstaged.push(FileChange::new(text[2..].to_string(), FileStatus::Untracked)),
            b'!' => st.ignored.push(text[2..].to_string()),
            _ => {}
        }
    }
    st
}

fn push_ordinary(st: &mut ParsedStatus, xy: &[u8], sub: &str, path: String, orig: Option<String>) {
    if xy.len() < 2 {
        return;
    }
    let submodule = sub.starts_with('S');
    if let Some(s) = FileStatus::from_code(xy[0]) {
        let mut fc = FileChange::new(path.clone(), s);
        fc.submodule = submodule;
        if matches!(s, FileStatus::Renamed | FileStatus::Copied) {
            fc.orig_path = orig.clone();
        }
        st.staged.push(fc);
    }
    if let Some(s) = FileStatus::from_code(xy[1]) {
        let mut fc = FileChange::new(path, s);
        fc.submodule = submodule;
        if matches!(s, FileStatus::Renamed | FileStatus::Copied) {
            fc.orig_path = orig;
        }
        st.unstaged.push(fc);
    }
}

fn parse_header(text: &str, st: &mut ParsedStatus) {
    let mut it = text.splitn(3, ' ');
    let _hash = it.next();
    let key = it.next().unwrap_or("");
    let value = it.next().unwrap_or("").to_string();
    match key {
        "branch.oid" => {
            if value == "(initial)" {
                st.head.unborn = true;
            } else {
                st.head.oid = Some(value);
            }
        }
        "branch.head" => {
            if value == "(detached)" {
                st.head.detached = true;
            } else {
                st.head.branch = Some(value);
            }
        }
        "branch.upstream" => st.head.upstream = Some(value),
        "branch.ab" => {
            for part in value.split(' ') {
                if let Some(a) = part.strip_prefix('+') {
                    st.head.ahead = a.parse().ok();
                } else if let Some(b) = part.strip_prefix('-') {
                    st.head.behind = b.parse().ok();
                }
            }
        }
        "stash" => st.stash_count = value.parse().unwrap_or(0),
        _ => {}
    }
}

/// Parses `git diff --numstat -z` output into (path, additions, deletions, binary).
pub fn parse_numstat(raw: &[u8]) -> Vec<(String, Option<u32>, Option<u32>)> {
    let mut out = Vec::new();
    let fields: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
    let mut i = 0;
    while i < fields.len() {
        let rec = String::from_utf8_lossy(fields[i]).into_owned();
        i += 1;
        if rec.is_empty() {
            continue;
        }
        let mut parts = rec.splitn(3, '\t');
        let add = parts.next().unwrap_or("-");
        let del = parts.next().unwrap_or("-");
        let mut path = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            // rename: "<add>\t<del>\t\0<old>\0<new>\0"
            path = fields.get(i + 1).map(|f| String::from_utf8_lossy(f).into_owned()).unwrap_or_default();
            i += 2;
        }
        out.push((path, add.parse().ok(), del.parse().ok()));
    }
    out
}

pub fn apply_numstat(list: &mut [FileChange], stats: &[(String, Option<u32>, Option<u32>)]) {
    for fc in list.iter_mut() {
        if let Some((_, a, d)) = stats.iter().find(|(p, _, _)| *p == fc.path) {
            fc.additions = *a;
            fc.deletions = *d;
            fc.binary = a.is_none() && d.is_none();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_headers_and_entries() {
        let raw = b"# branch.oid 1234567890abcdef\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0# stash 3\0\
1 .M N... 100644 100644 100644 aaa aaa src/a b.rs\0\
1 A. N... 000000 100644 100644 000 bbb new.txt\0\
1 MM N... 100644 100644 100644 ccc ddd both.txt\0\
2 R. N... 100644 100644 100644 eee eee R100 renamed.txt\0old name.txt\0\
u UU N... 100644 100644 100644 100644 f1 f2 f3 conflict.txt\0\
? untracked dir/file.txt\0! ignored.log\0";
        let st = parse_status(raw);
        assert_eq!(st.head.oid.as_deref(), Some("1234567890abcdef"));
        assert_eq!(st.head.branch.as_deref(), Some("main"));
        assert_eq!(st.head.upstream.as_deref(), Some("origin/main"));
        assert_eq!(st.head.ahead, Some(2));
        assert_eq!(st.head.behind, Some(1));
        assert_eq!(st.stash_count, 3);

        let staged: Vec<_> = st.staged.iter().map(|f| (f.path.as_str(), f.status)).collect();
        assert_eq!(
            staged,
            vec![("new.txt", FileStatus::Added), ("both.txt", FileStatus::Modified), ("renamed.txt", FileStatus::Renamed)]
        );
        assert_eq!(st.staged[2].orig_path.as_deref(), Some("old name.txt"));

        let unstaged: Vec<_> = st.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
        assert_eq!(
            unstaged,
            vec![
                ("src/a b.rs", FileStatus::Modified),
                ("both.txt", FileStatus::Modified),
                ("untracked dir/file.txt", FileStatus::Untracked)
            ]
        );
        assert_eq!(st.conflicted.len(), 1);
        assert_eq!(st.conflicted[0].conflict.as_deref(), Some("UU"));
        assert_eq!(st.ignored, vec!["ignored.log".to_string()]);
    }

    #[test]
    fn parses_detached_and_unborn() {
        let st = parse_status(b"# branch.oid (initial)\0# branch.head (detached)\0");
        assert!(st.head.unborn);
        assert!(st.head.detached);
        assert!(st.head.branch.is_none());
        assert!(st.head.upstream.is_none());
        assert!(st.head.ahead.is_none());
    }

    #[test]
    fn parses_numstat_with_renames_and_binary() {
        let raw = b"3\t1\tsrc/a.rs\0-\t-\timg.png\0" .to_vec();
        let mut raw2 = raw.clone();
        raw2.extend_from_slice(b"5\t0\t\0old.txt\0new.txt\0");
        let s = parse_numstat(&raw2);
        assert_eq!(s[0], ("src/a.rs".to_string(), Some(3), Some(1)));
        assert_eq!(s[1], ("img.png".to_string(), None, None));
        assert_eq!(s[2], ("new.txt".to_string(), Some(5), Some(0)));
    }
}
