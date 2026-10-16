//! Unified diff parsing and partial (hunk / line) patch construction.
//!
//! Parsing works on raw bytes so that patches rebuilt for partial staging preserve the
//! exact file content (encodings, CRLF) — the UI only ever receives a lossy text copy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Context,
    Add,
    Del,
    NoNewline,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub binary: bool,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_rename: bool,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub submodule: bool,
    pub hunks: Vec<DiffHunk>,
    pub truncated: bool,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone)]
pub struct RawHunk {
    pub header: Vec<u8>,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// Lines including their leading marker (' ', '+', '-', '\\'), without the trailing '\n'.
    pub lines: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub struct RawFileDiff {
    pub header: Vec<Vec<u8>>,
    pub hunks: Vec<RawHunk>,
    pub binary: bool,
}

pub const MAX_DIFF_LINES: usize = 20_000;

/// Splits a multi-file `git diff` output into per-file chunks.
pub fn split_files(raw: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut pos = 0;
    for line in raw.split_inclusive(|b| *b == b'\n') {
        if line.starts_with(b"diff --git ") || line.starts_with(b"diff --cc ") || line.starts_with(b"diff --combined ") {
            starts.push(pos);
        }
        pos += line.len();
    }
    let mut out = Vec::new();
    for (i, s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(raw.len());
        out.push(&raw[*s..e]);
    }
    out
}

fn parse_range(s: &str) -> (u32, u32) {
    let mut it = s.splitn(2, ',');
    let start = it.next().unwrap_or("0").parse().unwrap_or(0);
    let count = it.next().map(|c| c.parse().unwrap_or(0)).unwrap_or(1);
    (start, count)
}

fn parse_hunk_header(line: &[u8]) -> Option<(u32, u32, u32, u32)> {
    let text = String::from_utf8_lossy(line);
    let rest = text.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let ranges = &rest[..end];
    let mut parts = ranges.split(' ');
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let (os, oc) = parse_range(old);
    let (ns, nc) = parse_range(new);
    Some((os, oc, ns, nc))
}

pub fn parse_raw(raw: &[u8]) -> RawFileDiff {
    let mut out = RawFileDiff::default();
    let mut current: Option<RawHunk> = None;
    for line in raw.split(|b| *b == b'\n') {
        if line.starts_with(b"@@ ") {
            if let Some(h) = current.take() {
                out.hunks.push(h);
            }
            if let Some((os, oc, ns, nc)) = parse_hunk_header(line) {
                current = Some(RawHunk { header: line.to_vec(), old_start: os, old_lines: oc, new_start: ns, new_lines: nc, lines: Vec::new() });
            }
            continue;
        }
        match current.as_mut() {
            Some(h) => {
                if line.is_empty() {
                    // A truly empty line can only appear at the very end of the output.
                    continue;
                }
                match line[0] {
                    b' ' | b'+' | b'-' | b'\\' => h.lines.push(line.to_vec()),
                    _ => {}
                }
            }
            None => {
                if line.starts_with(b"Binary files ") || line.starts_with(b"GIT binary patch") {
                    out.binary = true;
                }
                if !line.is_empty() {
                    out.header.push(line.to_vec());
                }
            }
        }
    }
    if let Some(h) = current.take() {
        out.hunks.push(h);
    }
    out
}

fn lossy(b: &[u8]) -> String {
    let b = b.strip_suffix(b"\r").unwrap_or(b);
    String::from_utf8_lossy(b).into_owned()
}

/// Converts a raw single-file diff into the DTO sent to the UI.
pub fn to_file_diff(raw: &RawFileDiff, path: &str) -> FileDiff {
    let mut fd = FileDiff { path: path.to_string(), binary: raw.binary, ..Default::default() };
    for h in &raw.header {
        let t = String::from_utf8_lossy(h);
        if t.starts_with("new file mode") {
            fd.is_new = true;
            fd.new_mode = t.rsplit(' ').next().map(str::to_string);
        } else if t.starts_with("deleted file mode") {
            fd.is_deleted = true;
            fd.old_mode = t.rsplit(' ').next().map(str::to_string);
        } else if let Some(m) = t.strip_prefix("old mode ") {
            fd.old_mode = Some(m.to_string());
        } else if let Some(m) = t.strip_prefix("new mode ") {
            fd.new_mode = Some(m.to_string());
        } else if let Some(p) = t.strip_prefix("rename from ") {
            fd.is_rename = true;
            fd.old_path = Some(p.to_string());
        } else if t.starts_with("Subproject commit") || t.contains("160000") {
            fd.submodule = true;
        }
    }
    let mut total = 0usize;
    for h in &raw.hunks {
        let mut old_no = h.old_start;
        let mut new_no = h.new_start;
        let mut lines = Vec::with_capacity(h.lines.len());
        for l in &h.lines {
            total += 1;
            if total > MAX_DIFF_LINES {
                fd.truncated = true;
                break;
            }
            let content = lossy(&l[1..]);
            let dl = match l[0] {
                b'+' => {
                    fd.additions += 1;
                    let d = DiffLine { kind: LineKind::Add, old_no: None, new_no: Some(new_no), content };
                    new_no += 1;
                    d
                }
                b'-' => {
                    fd.deletions += 1;
                    let d = DiffLine { kind: LineKind::Del, old_no: Some(old_no), new_no: None, content };
                    old_no += 1;
                    d
                }
                b'\\' => DiffLine { kind: LineKind::NoNewline, old_no: None, new_no: None, content: lossy(l) },
                _ => {
                    let d = DiffLine { kind: LineKind::Context, old_no: Some(old_no), new_no: Some(new_no), content };
                    old_no += 1;
                    new_no += 1;
                    d
                }
            };
            lines.push(dl);
        }
        fd.hunks.push(DiffHunk {
            header: lossy(&h.header),
            old_start: h.old_start,
            old_lines: h.old_lines,
            new_start: h.new_start,
            new_lines: h.new_lines,
            lines,
        });
        if fd.truncated {
            break;
        }
    }
    fd
}

/// Selection of lines inside one hunk. `lines == None` selects the whole hunk.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkSelection {
    pub hunk: usize,
    /// Hunk header as displayed; used to detect that the diff changed since it was shown.
    pub header: String,
    pub lines: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchDirection {
    /// Apply the selected changes forward (stage from an unstaged diff).
    Forward,
    /// The patch will be applied with `--reverse` (unstage from a staged diff, or discard).
    Reverse,
}

/// Builds a patch containing only the selected changes. Returns `None` if nothing is selected.
pub fn build_partial_patch(raw: &RawFileDiff, selections: &[HunkSelection], dir: PatchDirection) -> Result<Option<Vec<u8>>, String> {
    let mut out: Vec<u8> = Vec::new();
    for h in &raw.header {
        if h.starts_with(b"index ") || h.starts_with(b"similarity index") {
            continue;
        }
        out.extend_from_slice(h);
        out.push(b'\n');
    }
    let mut any = false;
    let mut delta: i64 = 0;
    let mut sorted: Vec<&HunkSelection> = selections.iter().collect();
    sorted.sort_by_key(|s| s.hunk);
    for sel in sorted {
        let h = raw.hunks.get(sel.hunk).ok_or_else(|| "Hunk inexistente; atualize o diff.".to_string())?;
        if lossy(&h.header) != sel.header {
            return Err("O diff mudou desde que foi exibido; atualize e tente novamente.".to_string());
        }
        let mut body: Vec<u8> = Vec::new();
        let (mut old_c, mut new_c, mut changes) = (0u32, 0u32, 0u32);
        let mut prev_emitted = false;
        for (idx, l) in h.lines.iter().enumerate() {
            let selected = match &sel.lines {
                None => true,
                Some(v) => v.contains(&idx),
            };
            let marker = l[0];
            let emit: Option<u8> = match (marker, selected, dir) {
                (b' ', _, _) => Some(b' '),
                (b'\\', _, _) => {
                    if prev_emitted {
                        body.extend_from_slice(l);
                        body.push(b'\n');
                    }
                    continue;
                }
                (b'+', true, _) | (b'-', true, _) => Some(marker),
                (b'-', false, PatchDirection::Forward) => Some(b' '),
                (b'+', false, PatchDirection::Forward) => None,
                (b'+', false, PatchDirection::Reverse) => Some(b' '),
                (b'-', false, PatchDirection::Reverse) => None,
                _ => None,
            };
            prev_emitted = emit.is_some();
            if let Some(m) = emit {
                match m {
                    b' ' => {
                        old_c += 1;
                        new_c += 1;
                    }
                    b'+' => {
                        new_c += 1;
                        changes += 1;
                    }
                    _ => {
                        old_c += 1;
                        changes += 1;
                    }
                }
                body.push(m);
                body.extend_from_slice(&l[1..]);
                body.push(b'\n');
            }
        }
        if changes == 0 {
            continue;
        }
        any = true;
        let old_start = h.old_start;
        let new_start = (old_start as i64 + delta).max(0) as u32;
        out.extend_from_slice(format!("@@ -{},{} +{},{} @@\n", old_start, old_c, new_start, new_c).as_bytes());
        out.extend_from_slice(&body);
        delta += new_c as i64 - old_c as i64;
    }
    Ok(if any { Some(out) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &[u8] = b"diff --git a/f.txt b/f.txt
index 111..222 100644
--- a/f.txt
+++ b/f.txt
@@ -1,4 +1,5 @@
 one
-two
+TWO
+extra
 three
 four
@@ -10,2 +11,2 @@ fn x
 ten
-eleven
+ELEVEN
";

    #[test]
    fn parses_hunks_with_line_numbers() {
        let raw = parse_raw(DIFF);
        assert_eq!(raw.hunks.len(), 2);
        let fd = to_file_diff(&raw, "f.txt");
        assert_eq!(fd.additions, 3);
        assert_eq!(fd.deletions, 2);
        let h = &fd.hunks[0];
        assert_eq!((h.old_start, h.old_lines, h.new_start, h.new_lines), (1, 4, 1, 5));
        assert_eq!(h.lines[1].kind, LineKind::Del);
        assert_eq!(h.lines[1].old_no, Some(2));
        assert_eq!(h.lines[3].new_no, Some(3));
        assert_eq!(fd.hunks[1].header, "@@ -10,2 +11,2 @@ fn x");
    }

    #[test]
    fn whole_hunk_patch_keeps_hunk() {
        let raw = parse_raw(DIFF);
        let sel = vec![HunkSelection { hunk: 1, header: "@@ -10,2 +11,2 @@ fn x".into(), lines: None }];
        let p = String::from_utf8(build_partial_patch(&raw, &sel, PatchDirection::Forward).unwrap().unwrap()).unwrap();
        assert!(p.starts_with("diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n"));
        assert!(p.contains("@@ -10,2 +10,2 @@\n ten\n-eleven\n+ELEVEN\n"));
    }

    #[test]
    fn forward_line_selection_converts_unselected() {
        let raw = parse_raw(DIFF);
        // select only "+extra" (index 3)
        let sel = vec![HunkSelection { hunk: 0, header: "@@ -1,4 +1,5 @@".into(), lines: Some(vec![3]) }];
        let p = String::from_utf8(build_partial_patch(&raw, &sel, PatchDirection::Forward).unwrap().unwrap()).unwrap();
        assert!(p.contains("@@ -1,4 +1,5 @@\n one\n two\n+extra\n three\n four\n"), "{p}");
    }

    #[test]
    fn reverse_line_selection_converts_unselected() {
        let raw = parse_raw(DIFF);
        // unstage only "-two" (index 1): unselected '+' become context, unselected '-' dropped
        let sel = vec![HunkSelection { hunk: 0, header: "@@ -1,4 +1,5 @@".into(), lines: Some(vec![1]) }];
        let p = String::from_utf8(build_partial_patch(&raw, &sel, PatchDirection::Reverse).unwrap().unwrap()).unwrap();
        assert!(p.contains("@@ -1,6 +1,5 @@\n one\n-two\n TWO\n extra\n three\n four\n"), "{p}");
    }

    #[test]
    fn detects_stale_selection_and_empty() {
        let raw = parse_raw(DIFF);
        let stale = vec![HunkSelection { hunk: 0, header: "@@ -1,3 +1,3 @@".into(), lines: None }];
        assert!(build_partial_patch(&raw, &stale, PatchDirection::Forward).is_err());
        let none = vec![HunkSelection { hunk: 0, header: "@@ -1,4 +1,5 @@".into(), lines: Some(vec![0]) }];
        assert!(build_partial_patch(&raw, &none, PatchDirection::Forward).unwrap().is_none());
    }

    #[test]
    fn splits_multi_file_output() {
        let mut two = DIFF.to_vec();
        two.extend_from_slice(b"diff --git a/g b/g\nBinary files a/g and b/g differ\n");
        let files = split_files(&two);
        assert_eq!(files.len(), 2);
        assert!(parse_raw(files[1]).binary);
    }
}
