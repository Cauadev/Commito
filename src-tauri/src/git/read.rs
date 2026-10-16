//! Read-only repository queries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::cache::{fingerprint, GraphCache};
use super::diff::{self, FileDiff};
use super::graph::{layout, GraphNode};
use super::log::{self, CommitDetail, CommitInfo, GraphRow, RowKind};
use super::refs::{self, GitRef, OperationState, StashEntry, Worktree};
use super::runner::{GitRunner, RunOpts};
use super::status::{self, FileChange, HeadInfo};
use crate::errors::{AppError, AppResult, ErrorCode};
use crate::state::{native_path, Repo};

pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRepo {
    pub path: String,
    pub name: String,
    pub git_dir: String,
    pub common_dir: String,
    pub bare: bool,
    pub linked_worktree: bool,
}

/// Validates that `path` is (inside) a git repository and returns its root.
pub async fn resolve(git: &GitRunner, path: &Path) -> AppResult<ResolvedRepo> {
    if !path.is_dir() {
        return Err(AppError::new(ErrorCode::NotFound, "A pasta não existe.").details(path.display().to_string()));
    }
    let out = git
        .run(path, &["rev-parse", "--is-bare-repository", "--absolute-git-dir", "--git-common-dir"], RunOpts::read())
        .await?;
    if !out.ok() {
        return Err(AppError::new(ErrorCode::NotARepository, "A pasta selecionada não é um repositório Git.")
            .details(out.stderr)
            .recovery("Escolha a pasta raiz de um repositório ou use \"Inicializar repositório\"."));
    }
    let text = out.text();
    let mut lines = text.lines();
    let bare = lines.next() == Some("true");
    let git_dir = lines.next().unwrap_or("").to_string();
    let common_raw = lines.next().unwrap_or("").to_string();
    let common_dir = {
        let p = PathBuf::from(&common_raw);
        let p = if p.is_absolute() { p } else { path.join(p) };
        std::fs::canonicalize(&p).map(|c| strip_unc(&c)).unwrap_or_else(|_| p.display().to_string())
    };
    let root = if bare {
        git_dir.clone()
    } else {
        let top = git.read(path, &["rev-parse", "--show-toplevel"]).await?;
        top.trim().to_string()
    };
    let root = native_path(&root);
    let name = Path::new(&root).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.clone());
    let linked = !bare && !crate::settings::same_path(&native_path(&git_dir), &common_dir);
    Ok(ResolvedRepo { path: root, name, git_dir: native_path(&git_dir), common_dir: native_path(&common_dir), bare, linked_worktree: linked })
}

fn strip_unc(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

pub async fn git_dir(repo: &Repo) -> AppResult<PathBuf> {
    let d = repo.git.read(&repo.path, &["rev-parse", "--absolute-git-dir"]).await?;
    Ok(PathBuf::from(native_path(d.trim())))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub head: HeadInfo,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub conflicted: Vec<FileChange>,
    pub ignored: Vec<String>,
    pub stash_count: u32,
    pub operation: Option<OperationState>,
    pub bare: bool,
}

pub async fn status(repo: &Repo, include_ignored: bool) -> AppResult<GitStatus> {
    let mut args = vec!["status", "--porcelain=v2", "-z", "--branch", "--show-stash", "--untracked-files=all", "--find-renames"];
    if include_ignored {
        args.push("--ignored=matching");
    }
    let st_fut = repo.git.run(&repo.path, &args, RunOpts::read());
    let un_fut = repo.git.run(&repo.path, &["diff", "--numstat", "-z", "--no-ext-diff", "--no-renames"], RunOpts::read());
    let sg_fut = repo.git.run(&repo.path, &["diff", "--cached", "--numstat", "-z", "--no-ext-diff", "-M"], RunOpts::read());
    let (st, un, sg) = tokio::join!(st_fut, un_fut, sg_fut);
    let st = st?;
    if !st.ok() {
        if st.stderr.contains("must be run in a work tree") {
            return Ok(GitStatus { head: HeadInfo::default(), staged: vec![], unstaged: vec![], conflicted: vec![], ignored: vec![], stash_count: 0, operation: None, bare: true });
        }
        return Err(crate::errors::classify_git_failure(&["status".into()], &st.stderr).repo(&repo.display));
    }
    let mut parsed = status::parse_status(&st.stdout);
    if let Ok(un) = un {
        if un.ok() {
            status::apply_numstat(&mut parsed.unstaged, &status::parse_numstat(&un.stdout));
        }
    }
    if let Ok(sg) = sg {
        if sg.ok() {
            status::apply_numstat(&mut parsed.staged, &status::parse_numstat(&sg.stdout));
        }
    }
    let operation = match git_dir(repo).await {
        Ok(d) => refs::detect_operation(&d),
        Err(_) => None,
    };
    Ok(GitStatus {
        head: parsed.head,
        staged: parsed.staged,
        unstaged: parsed.unstaged,
        conflicted: parsed.conflicted,
        ignored: parsed.ignored,
        stash_count: parsed.stash_count,
        operation,
        bare: false,
    })
}

pub async fn list_refs(repo: &Repo) -> AppResult<Vec<GitRef>> {
    let raw = repo.git.read(&repo.path, &["for-each-ref", refs::REF_FORMAT, "refs/heads", "refs/remotes", "refs/tags"]).await?;
    Ok(refs::parse_refs(&raw))
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeadRef {
    pub oid: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
}

pub async fn head_ref(repo: &Repo) -> HeadRef {
    let oid = repo.git.run(&repo.path, &["rev-parse", "--verify", "-q", "HEAD"], RunOpts::read()).await.ok().filter(|o| o.ok()).map(|o| o.text().trim().to_string());
    let branch = repo.git.run(&repo.path, &["symbolic-ref", "-q", "--short", "HEAD"], RunOpts::read()).await.ok().filter(|o| o.ok()).map(|o| o.text().trim().to_string());
    HeadRef { detached: branch.is_none() && oid.is_some(), oid, branch }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphPayload {
    pub rows: Vec<GraphRow>,
    pub refs: Vec<GitRef>,
    pub head: HeadRef,
    pub has_more: bool,
    pub max_lanes: usize,
    pub stashes: Vec<StashEntry>,
    pub total_loaded: usize,
}

fn scope_args(scope: &str, has_head: bool) -> Vec<String> {
    let mut v: Vec<String> = match scope {
        "current" => vec![],
        "local" => vec!["--branches".into(), "--tags".into()],
        "remote" => vec!["--remotes".into()],
        _ => vec!["--branches".into(), "--remotes".into(), "--tags".into()],
    };
    if has_head && scope != "remote" {
        v.push("HEAD".into());
    }
    v
}

pub async fn list_stashes(repo: &Repo) -> AppResult<Vec<StashEntry>> {
    let out = repo.git.run(&repo.path, &["stash", "list", "-z", refs::STASH_FORMAT], RunOpts::read()).await?;
    if !out.ok() {
        return Ok(vec![]);
    }
    Ok(refs::parse_stashes(&out.stdout))
}

pub async fn graph(
    repo: &Repo,
    cache: &tokio::sync::Mutex<HashMap<String, GraphCache>>,
    scope: &str,
    limit: usize,
    include_wip: bool,
    reset: bool,
) -> AppResult<GraphPayload> {
    let refs_raw = repo.git.read(&repo.path, &["for-each-ref", refs::REF_FORMAT, "refs/heads", "refs/remotes", "refs/tags"]).await?;
    let head = head_ref(repo).await;
    let refs = refs::parse_refs(&refs_raw);
    let fp = fingerprint(&[&refs_raw, head.oid.as_deref().unwrap_or(""), head.branch.as_deref().unwrap_or("")]);
    let stashes = list_stashes(repo).await.unwrap_or_default();
    let limit = limit.clamp(100, 200_000);
    let key = format!("{}|{}", repo.key, scope);

    let mut guard = cache.lock().await;
    let entry = guard.entry(key).or_default();
    if reset || entry.fingerprint != fp {
        *entry = GraphCache { fingerprint: fp, ..Default::default() };
    }
    let has_refs = head.oid.is_some() || !refs.is_empty();
    if !has_refs {
        entry.exhausted = true;
    }
    while !entry.exhausted && entry.commits.len() < limit {
        let want = limit - entry.commits.len();
        let mut args: Vec<String> = vec![
            "log".into(),
            "--date-order".into(),
            "-z".into(),
            log::LOG_FORMAT.into(),
            format!("--max-count={want}"),
            format!("--skip={}", entry.commits.len()),
        ];
        let sa = scope_args(scope, head.oid.is_some());
        if sa.is_empty() && head.oid.is_none() {
            entry.exhausted = true;
            break;
        }
        if sa.is_empty() {
            args.push("HEAD".into());
        } else {
            args.extend(sa);
        }
        args.push("--".into());
        let out = repo.git.run(&repo.path, &args, RunOpts::read()).await?;
        if !out.ok() {
            if out.stderr.contains("does not have any commits") || out.stderr.contains("bad default revision") {
                entry.exhausted = true;
                break;
            }
            return Err(crate::errors::classify_git_failure(&args, &out.stderr));
        }
        let batch = log::parse_log(&out.stdout);
        if batch.len() < want {
            entry.exhausted = true;
        }
        if batch.is_empty() {
            break;
        }
        entry.commits.extend(batch);
    }

    let commits: &[CommitInfo] = &entry.commits;
    let loaded: std::collections::HashSet<&str> = commits.iter().map(|c| c.hash.as_str()).collect();

    // Assemble display order: WIP, then commits with stash nodes inserted by time.
    enum Item<'a> {
        Wip(String),
        Commit(&'a CommitInfo),
        Stash(&'a StashEntry, Vec<String>),
    }
    let mut items: Vec<Item> = Vec::with_capacity(commits.len() + 1 + stashes.len());
    if include_wip {
        if let Some(h) = head.oid.as_deref().filter(|h| loaded.contains(h)) {
            items.push(Item::Wip(h.to_string()));
        }
    }
    let mut pending_stashes: Vec<&StashEntry> = if scope == "all" || scope == "local" || scope == "current" {
        stashes.iter().filter(|s| s.base.as_deref().is_some_and(|b| loaded.contains(b))).collect()
    } else {
        vec![]
    };
    pending_stashes.sort_by(|a, b| b.time.cmp(&a.time));
    let mut si = 0;
    for c in commits {
        while si < pending_stashes.len() {
            let s = pending_stashes[si];
            // Place the stash before its base and before older commits.
            if s.time >= c.committer_time || s.base.as_deref() == Some(c.hash.as_str()) {
                items.push(Item::Stash(s, vec![s.base.clone().unwrap_or_default()]));
                si += 1;
            } else {
                break;
            }
        }
        items.push(Item::Commit(c));
    }

    let wip_parents: Vec<String> = head.oid.iter().cloned().collect();
    let nodes: Vec<GraphNode> = items
        .iter()
        .map(|it| match it {
            Item::Wip(_) => GraphNode { hash: "WIP", parents: &wip_parents, virtual_node: true },
            Item::Commit(c) => GraphNode { hash: &c.hash, parents: &c.parents, virtual_node: false },
            Item::Stash(s, p) => GraphNode { hash: &s.hash, parents: p, virtual_node: true },
        })
        .collect();
    let lay = layout(&nodes);

    let rows: Vec<GraphRow> = items
        .iter()
        .zip(lay.rows)
        .map(|(it, l)| match it {
            Item::Wip(h) => GraphRow {
                kind: RowKind::Wip,
                hash: "WIP".into(),
                parents: vec![h.clone()],
                subject: String::new(),
                author_name: String::new(),
                author_email: String::new(),
                author_time: crate::settings::now_secs(),
                committer_time: crate::settings::now_secs(),
                lane: l.lane,
                color: l.color,
                edges: l.edges,
                stash_ref: None,
            },
            Item::Commit(c) => GraphRow {
                kind: RowKind::Commit,
                hash: c.hash.clone(),
                parents: c.parents.clone(),
                subject: c.subject.clone(),
                author_name: c.author_name.clone(),
                author_email: c.author_email.clone(),
                author_time: c.author_time,
                committer_time: c.committer_time,
                lane: l.lane,
                color: l.color,
                edges: l.edges,
                stash_ref: None,
            },
            Item::Stash(s, p) => GraphRow {
                kind: RowKind::Stash,
                hash: s.hash.clone(),
                parents: p.clone(),
                subject: s.message.clone(),
                author_name: String::new(),
                author_email: String::new(),
                author_time: s.time,
                committer_time: s.time,
                lane: l.lane,
                color: l.color,
                edges: l.edges,
                stash_ref: Some(s.reference.clone()),
            },
        })
        .collect();

    Ok(GraphPayload {
        total_loaded: entry.commits.len(),
        has_more: !entry.exhausted,
        rows,
        refs,
        head,
        max_lanes: lay.max_lanes,
        stashes,
    })
}

pub async fn search_commits(repo: &Repo, query: &str, scope: &str, max: usize) -> AppResult<Vec<String>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let head = head_ref(repo).await;
    let base: Vec<String> = vec!["log".into(), "--format=%H".into(), format!("--max-count={max}"), "-i".into()];
    let sa = scope_args(scope, head.oid.is_some());
    let revs = if sa.is_empty() { vec!["HEAD".to_string()] } else { sa };
    let mut found: Vec<String> = Vec::new();
    let push_unique = |text: String, found: &mut Vec<String>| {
        for l in text.lines() {
            if !l.is_empty() && !found.iter().any(|f| f == l) {
                found.push(l.to_string());
            }
        }
    };
    if q.len() >= 4 && q.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(o) = repo.git.run(&repo.path, &["rev-parse", "--verify", "-q", &format!("{q}^{{commit}}")], RunOpts::read()).await {
            if o.ok() {
                push_unique(o.text(), &mut found);
            }
        }
    }
    for flag in [format!("--grep={q}"), format!("--author={q}")] {
        let mut args = base.clone();
        args.push("--fixed-strings".into());
        args.push(flag);
        args.extend(revs.iter().cloned());
        args.push("--".into());
        if let Ok(o) = repo.git.run(&repo.path, &args, RunOpts::read()).await {
            if o.ok() {
                push_unique(o.text(), &mut found);
            }
        }
    }
    found.truncate(max);
    Ok(found)
}

pub async fn commit_detail(repo: &Repo, sha: &str) -> AppResult<CommitDetail> {
    validate_rev(sha)?;
    let raw = repo.git.run_ok(&repo.path, &["show", "-s", log::DETAIL_FORMAT, sha, "--"], RunOpts::read()).await?;
    let mut detail = log::parse_detail(&raw.stdout).ok_or_else(|| AppError::internal("Não foi possível ler o commit."))?;
    detail.files = commit_files(repo, &detail.hash, detail.parents.first().map(String::as_str)).await?;
    Ok(detail)
}

/// Files changed by `sha` relative to `base` (first parent by default, empty tree for roots).
pub async fn commit_files(repo: &Repo, sha: &str, base: Option<&str>) -> AppResult<Vec<FileChange>> {
    let base = base.unwrap_or(EMPTY_TREE);
    let ns = repo.git.run_ok(&repo.path, &["diff", "--name-status", "-z", "-M", "--no-ext-diff", base, sha, "--"], RunOpts::read()).await?;
    let mut files = log::parse_name_status(&ns.stdout);
    if let Ok(num) = repo.git.run(&repo.path, &["diff", "--numstat", "-z", "-M", "--no-ext-diff", base, sha, "--"], RunOpts::read()).await {
        if num.ok() {
            status::apply_numstat(&mut files, &status::parse_numstat(&num.stdout));
        }
    }
    Ok(files)
}

pub async fn stash_untracked_files(repo: &Repo, stash_hash: &str) -> AppResult<Vec<FileChange>> {
    validate_rev(stash_hash)?;
    let third = format!("{stash_hash}^3");
    let ok = repo.git.run(&repo.path, &["rev-parse", "--verify", "-q", &third], RunOpts::read()).await?;
    if !ok.ok() {
        return Ok(vec![]);
    }
    let mut files = commit_files(repo, &third, None).await?;
    for f in files.iter_mut() {
        f.status = status::FileStatus::Untracked;
    }
    Ok(files)
}

pub fn validate_rev(rev: &str) -> AppResult<()> {
    if rev.is_empty() || rev.starts_with('-') || rev.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::invalid("Revisão inválida."));
    }
    Ok(())
}

pub fn validate_path(p: &str) -> AppResult<()> {
    if p.is_empty() || p.contains('\0') {
        return Err(AppError::invalid("Caminho inválido."));
    }
    let norm = p.replace('\\', "/");
    if norm.starts_with('/') || norm.split('/').any(|seg| seg == "..") || (norm.len() > 1 && norm.as_bytes()[1] == b':') {
        return Err(AppError::invalid("O caminho precisa ser relativo ao repositório."));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffSource {
    /// Working tree vs index.
    Unstaged,
    /// Index vs HEAD.
    Staged,
    /// Untracked file shown as all-new.
    Untracked,
}

pub fn diff_context_arg(ctx: u32) -> String {
    format!("-U{}", ctx.min(1000))
}

pub async fn raw_working_diff(repo: &Repo, path: &str, source: DiffSource, ctx: u32) -> AppResult<Vec<u8>> {
    validate_path(path)?;
    let u = diff_context_arg(ctx);
    let out = match source {
        DiffSource::Unstaged => repo.git.run(&repo.path, &["diff", "--no-ext-diff", "--no-color", &u, "--", path], RunOpts::read()).await?,
        DiffSource::Staged => repo.git.run(&repo.path, &["diff", "--cached", "--no-ext-diff", "--no-color", "-M", &u, "--", path], RunOpts::read()).await?,
        DiffSource::Untracked => {
            let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
            repo.git.run(&repo.path, &["diff", "--no-index", "--no-ext-diff", "--no-color", &u, "--", null, path], RunOpts::read()).await?
        }
    };
    // `--no-index` exits 1 when files differ.
    if out.code > 1 || (out.code == 1 && source != DiffSource::Untracked) {
        return Err(crate::errors::classify_git_failure(&["diff".into()], &out.stderr));
    }
    Ok(out.stdout)
}

pub async fn working_diff(repo: &Repo, path: &str, source: DiffSource, ctx: u32) -> AppResult<FileDiff> {
    let raw = raw_working_diff(repo, path, source, ctx).await?;
    let parsed = diff::parse_raw(&raw);
    let mut fd = diff::to_file_diff(&parsed, path);
    if source == DiffSource::Untracked {
        fd.is_new = true;
    }
    if raw.is_empty() {
        // e.g. pure mode change hidden, or submodule; return empty diff
        fd.hunks.clear();
    }
    Ok(fd)
}

pub async fn commit_file_diff(repo: &Repo, base: Option<&str>, sha: &str, path: &str, ctx: u32) -> AppResult<FileDiff> {
    validate_rev(sha)?;
    validate_path(path)?;
    let base = match base {
        Some(b) => {
            validate_rev(b)?;
            b.to_string()
        }
        None => {
            let d = repo.git.read(&repo.path, &["rev-list", "--parents", "-n", "1", sha]).await?;
            d.split_whitespace().nth(1).map(str::to_string).unwrap_or_else(|| EMPTY_TREE.to_string())
        }
    };
    let u = diff_context_arg(ctx);
    let out = repo
        .git
        .run_ok(&repo.path, &["diff", "--no-ext-diff", "--no-color", "-M", &u, &base, sha, "--", path], RunOpts::read())
        .await?;
    let files = diff::split_files(&out.stdout);
    let raw = files.first().map(|f| diff::parse_raw(f)).unwrap_or_default();
    Ok(diff::to_file_diff(&raw, path))
}

pub async fn file_history(repo: &Repo, path: &str, max: usize) -> AppResult<Vec<CommitInfo>> {
    validate_path(path)?;
    let out = repo
        .git
        .run_ok(&repo.path, &["log", "--follow", "-z", log::LOG_FORMAT, &format!("--max-count={max}"), "--", path], RunOpts::read())
        .await?;
    Ok(log::parse_log(&out.stdout))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line: u32,
    pub hash: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub hash: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blame {
    pub lines: Vec<BlameLine>,
    pub commits: HashMap<String, BlameCommit>,
}

pub fn parse_blame(raw: &[u8]) -> Blame {
    let mut lines = Vec::new();
    let mut commits: HashMap<String, BlameCommit> = HashMap::new();
    let mut current: Option<(String, u32)> = None;
    for line in raw.split(|b| *b == b'\n') {
        if let Some(content) = line.strip_prefix(b"\t") {
            if let Some((hash, n)) = current.take() {
                let content = String::from_utf8_lossy(content.strip_suffix(b"\r").unwrap_or(content)).into_owned();
                lines.push(BlameLine { line: n, hash, content });
            }
            continue;
        }
        let text = String::from_utf8_lossy(line);
        let mut parts = text.split(' ');
        let first = parts.next().unwrap_or("");
        if first.len() >= 40 && first.chars().all(|c| c.is_ascii_hexdigit()) {
            let _orig = parts.next();
            let fin: u32 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            current = Some((first.to_string(), fin));
            commits.entry(first.to_string()).or_insert_with(|| BlameCommit { hash: first.to_string(), author: String::new(), time: 0, summary: String::new() });
            continue;
        }
        if let Some((hash, _)) = &current {
            if let Some(c) = commits.get_mut(hash) {
                if let Some(v) = text.strip_prefix("author ") {
                    c.author = v.to_string();
                } else if let Some(v) = text.strip_prefix("author-time ") {
                    c.time = v.parse().unwrap_or(0);
                } else if let Some(v) = text.strip_prefix("summary ") {
                    c.summary = v.to_string();
                }
            }
        }
    }
    Blame { lines, commits }
}

pub async fn blame(repo: &Repo, path: &str, rev: Option<&str>) -> AppResult<Blame> {
    validate_path(path)?;
    let mut args: Vec<String> = vec!["blame".into(), "--porcelain".into()];
    if let Some(r) = rev {
        validate_rev(r)?;
        args.push(r.to_string());
    }
    args.push("--".into());
    args.push(path.to_string());
    let out = repo.git.run_ok(&repo.path, &args, RunOpts::read()).await?;
    Ok(parse_blame(&out.stdout))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub name: String,
    pub fetch_url: Option<String>,
    pub push_url: Option<String>,
    pub web_url: Option<String>,
}

pub async fn list_remotes(repo: &Repo) -> AppResult<Vec<Remote>> {
    let raw = repo.git.read(&repo.path, &["remote", "-v"]).await?;
    let mut map: Vec<Remote> = Vec::new();
    for line in raw.lines() {
        let mut it = line.split_whitespace();
        let (Some(name), Some(url), Some(kind)) = (it.next(), it.next(), it.next()) else { continue };
        let idx = match map.iter().position(|r| r.name == name) {
            Some(i) => i,
            None => {
                map.push(Remote { name: name.into(), fetch_url: None, push_url: None, web_url: None });
                map.len() - 1
            }
        };
        let url = crate::redact::redact(url);
        if kind.contains("fetch") {
            map[idx].web_url = remote_web_url(&url);
            map[idx].fetch_url = Some(url);
        } else {
            map[idx].push_url = Some(url);
        }
    }
    Ok(map)
}

/// Converts a git remote URL into a browsable https URL when possible.
pub fn remote_web_url(url: &str) -> Option<String> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.strip_prefix("git@").or_else(|| url.strip_prefix("ssh://git@")) {
        // git@host:owner/repo.git  |  ssh://git@host:22/owner/repo.git
        let (host, path) = if url.starts_with("ssh://") {
            let (h, p) = rest.split_once('/')?;
            (h.split(':').next()?.to_string(), p.to_string())
        } else {
            let (h, p) = rest.split_once(':')?;
            (h.to_string(), p.to_string())
        };
        (host, path)
    } else if url.starts_with("https://") || url.starts_with("http://") {
        let u = url::Url::parse(url).ok()?;
        (u.host_str()?.to_string(), u.path().trim_start_matches('/').to_string())
    } else {
        return None;
    };
    let host = host.trim_start_matches("ssh.").replace("altssh.", "");
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    Some(format!("https://{host}/{path}"))
}

pub async fn list_worktrees(repo: &Repo) -> AppResult<Vec<Worktree>> {
    let raw = repo.git.read(&repo.path, &["worktree", "list", "--porcelain"]).await?;
    let mut list = refs::parse_worktrees(&raw);
    let futs = list.iter().map(|w| {
        let git = repo.git.clone();
        let path = PathBuf::from(native_path(&w.path));
        let skip = w.bare || w.prunable || !path.is_dir();
        async move {
            if skip {
                return None;
            }
            let o = git.run(&path, &["status", "--porcelain=v2", "-z", "--untracked-files=normal"], RunOpts::read()).await.ok()?;
            if !o.ok() {
                return None;
            }
            Some(o.stdout.split(|b| *b == 0).filter(|r| !r.is_empty() && r[0] != b'#').count() as u32)
        }
    });
    let results = futures_join_all(futs).await;
    for (w, r) in list.iter_mut().zip(results) {
        w.path = native_path(&w.path);
        w.is_current = crate::settings::same_path(&w.path, &repo.display);
        w.changed_files = r;
        w.dirty = r.map(|n| n > 0);
    }
    Ok(list)
}

/// Minimal join_all to avoid pulling the `futures` crate.
pub async fn futures_join_all<F: std::future::Future>(futs: impl IntoIterator<Item = F>) -> Vec<F::Output>
where
    F::Output: Send + 'static,
    F: Send + 'static,
{
    let handles: Vec<_> = futs.into_iter().map(tokio::spawn).collect();
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        out.push(h.await.expect("task panicked"));
    }
    out
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub local_name: Option<String>,
    pub local_email: Option<String>,
    pub global_name: Option<String>,
    pub global_email: Option<String>,
    pub effective_name: Option<String>,
    pub effective_email: Option<String>,
    pub gpg_sign: bool,
    pub signing_key: Option<String>,
    pub pull_rebase: Option<String>,
    pub pull_ff: Option<String>,
}

async fn cfg(git: &GitRunner, cwd: &Path, args: &[&str]) -> Option<String> {
    let mut a = vec!["config"];
    a.extend_from_slice(args);
    let o = git.run(cwd, &a, RunOpts::read()).await.ok()?;
    o.ok().then(|| o.text().trim().to_string()).filter(|s| !s.is_empty())
}

pub async fn identity(repo: &Repo) -> Identity {
    let g = &repo.git;
    let p = &repo.path;
    Identity {
        local_name: cfg(g, p, &["--local", "--get", "user.name"]).await,
        local_email: cfg(g, p, &["--local", "--get", "user.email"]).await,
        global_name: cfg(g, p, &["--global", "--get", "user.name"]).await,
        global_email: cfg(g, p, &["--global", "--get", "user.email"]).await,
        effective_name: cfg(g, p, &["--get", "user.name"]).await,
        effective_email: cfg(g, p, &["--get", "user.email"]).await,
        gpg_sign: cfg(g, p, &["--bool", "--get", "commit.gpgsign"]).await.as_deref() == Some("true"),
        signing_key: cfg(g, p, &["--get", "user.signingkey"]).await,
        pull_rebase: cfg(g, p, &["--get", "pull.rebase"]).await,
        pull_ff: cfg(g, p, &["--get", "pull.ff"]).await,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    pub path: String,
    pub hash: String,
    pub state: String,
    pub describe: Option<String>,
}

pub async fn submodules(repo: &Repo) -> AppResult<Vec<Submodule>> {
    if !repo.path.join(".gitmodules").is_file() {
        return Ok(vec![]);
    }
    let out = repo.git.run(&repo.path, &["submodule", "status"], RunOpts::read()).await?;
    if !out.ok() {
        return Ok(vec![]);
    }
    Ok(out
        .text()
        .lines()
        .filter_map(|l| {
            let state = match l.chars().next()? {
                '-' => "uninitialized",
                '+' => "modified",
                'U' => "conflict",
                _ => "ok",
            };
            let rest = l.get(1..)?;
            let mut it = rest.splitn(2, ' ');
            let hash = it.next()?.to_string();
            let rest = it.next().unwrap_or("");
            let (path, describe) = match rest.rsplit_once(" (") {
                Some((p, d)) => (p.to_string(), Some(d.trim_end_matches(')').to_string())),
                None => (rest.to_string(), None),
            };
            Some(Submodule { path, hash, state: state.into(), describe })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_urls() {
        assert_eq!(remote_web_url("git@github.com:me/app.git").as_deref(), Some("https://github.com/me/app"));
        assert_eq!(remote_web_url("https://gitlab.com/g/sub/p.git").as_deref(), Some("https://gitlab.com/g/sub/p"));
        assert_eq!(remote_web_url("ssh://git@bitbucket.org:22/ws/repo.git").as_deref(), Some("https://bitbucket.org/ws/repo"));
        assert_eq!(remote_web_url("https://bob@bitbucket.org/ws/repo.git").as_deref(), Some("https://bitbucket.org/ws/repo"));
        assert_eq!(remote_web_url("C:/local/repo"), None);
    }

    #[test]
    fn path_validation() {
        assert!(validate_path("src/a.rs").is_ok());
        assert!(validate_path("-weird name.txt").is_ok());
        assert!(validate_path("../escape").is_err());
        assert!(validate_path("C:/abs").is_err());
        assert!(validate_path("/abs").is_err());
        assert!(validate_rev("--exec=x").is_err());
        assert!(validate_rev("abc123").is_ok());
    }

    #[test]
    fn parses_blame_porcelain() {
        let sha = "a".repeat(40);
        let raw = format!("{sha} 1 1 2\nauthor Ana\nauthor-time 100\nsummary first\nfilename f\n\tline one\n{sha} 2 2\n\tline two\n");
        let b = parse_blame(raw.as_bytes());
        assert_eq!(b.lines.len(), 2);
        assert_eq!(b.lines[1].content, "line two");
        assert_eq!(b.commits[&sha].author, "Ana");
        assert_eq!(b.commits[&sha].summary, "first");
    }
}
