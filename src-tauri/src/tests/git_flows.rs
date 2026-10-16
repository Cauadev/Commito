//! End-to-end tests against real temporary repositories using the system git.

use std::collections::HashMap;
use std::path::Path;

use crate::git::diff::HunkSelection;
use crate::git::ops::{self, CheckoutTarget, CommitRequest, LineAction, MergeMode, SeqAction, StashRequest, WorktreeRequest};
use crate::git::read::{self, DiffSource};
use crate::git::refs::OperationKind;
use crate::git::runner::{discover_git, GitRunner, RunOpts};
use crate::state::{native_path, norm_key, Repo};

async fn setup() -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    let git = GitRunner::new(discover_git(None).expect("git must be installed for integration tests"));
    let path = dir.path().to_path_buf();
    git.run_ok(&path, &["init", "-q", "--initial-branch=main"], RunOpts::write()).await.unwrap();
    for (k, v) in [("user.name", "Test"), ("user.email", "t@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git.run_ok(&path, &["config", k, v], RunOpts::write()).await.unwrap();
    }
    let display = path.display().to_string();
    (dir, Repo { git, key: norm_key(&display), display, path })
}

fn write(repo: &Repo, name: &str, content: &str) {
    let p = repo.path.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, content).unwrap();
}

async fn commit_all(repo: &Repo, msg: &str) {
    ops::stage(repo, &[]).await.unwrap();
    let req = CommitRequest { message: msg.into(), amend: false, sign_off: false, gpg_sign: Some(false), no_verify: true, allow_empty: false };
    ops::commit(repo, &req).await.unwrap();
}

#[tokio::test]
async fn graph_of_branch_and_merge() {
    let (_d, repo) = setup().await;
    write(&repo, "a.txt", "1\n");
    commit_all(&repo, "base").await;
    ops::create_branch(&repo, "feature", None, true).await.unwrap();
    write(&repo, "f.txt", "feature\n");
    commit_all(&repo, "feature work").await;
    ops::checkout(&repo, &CheckoutTarget::Branch { name: "main".into() }, false).await.unwrap();
    write(&repo, "a.txt", "1\n2\n");
    commit_all(&repo, "main work").await;
    let res = ops::merge(&repo, "feature", MergeMode::NoFf).await.unwrap();
    assert!(res.ok, "{res:?}");

    let cache = tokio::sync::Mutex::new(HashMap::new());
    let g = read::graph(&repo, &cache, "all", 1000, false, false).await.unwrap();
    assert_eq!(g.rows.len(), 4);
    assert_eq!(g.rows[0].parents.len(), 2, "first row is the merge commit");
    assert_eq!(g.max_lanes, 2);
    assert!(g.refs.iter().any(|r| r.short == "feature"));
    assert_eq!(g.head.branch.as_deref(), Some("main"));

    // WIP row appears on top and connects to HEAD
    write(&repo, "a.txt", "changed\n");
    let g = read::graph(&repo, &cache, "all", 1000, true, false).await.unwrap();
    assert_eq!(g.rows[0].hash, "WIP");
    assert_eq!(g.rows.len(), 5);

    // file history and blame
    let hist = read::file_history(&repo, "a.txt", 50).await.unwrap();
    assert_eq!(hist.len(), 2);
    let blame = read::blame(&repo, "a.txt", Some("HEAD")).await.unwrap();
    assert_eq!(blame.lines.len(), 2);
}

#[tokio::test]
async fn line_level_staging_and_unstaging() {
    let (_d, repo) = setup().await;
    let base: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    write(&repo, "f.txt", &base);
    commit_all(&repo, "base").await;
    let changed = base.replace("line 2\n", "line 2 changed\n").replace("line 25\n", "line 25 changed\n");
    write(&repo, "f.txt", &changed);

    let diff = read::working_diff(&repo, "f.txt", DiffSource::Unstaged, 3).await.unwrap();
    assert_eq!(diff.hunks.len(), 2);
    // Stage only the second hunk.
    let sel = vec![HunkSelection { hunk: 1, header: diff.hunks[1].header.clone(), lines: None }];
    ops::apply_lines(&repo, "f.txt", LineAction::Stage, &sel, 3).await.unwrap();
    let staged = read::working_diff(&repo, "f.txt", DiffSource::Staged, 3).await.unwrap();
    assert_eq!(staged.hunks.len(), 1);
    assert!(staged.hunks[0].lines.iter().any(|l| l.content == "line 25 changed"));
    let unstaged = read::working_diff(&repo, "f.txt", DiffSource::Unstaged, 3).await.unwrap();
    assert_eq!(unstaged.hunks.len(), 1);
    assert!(unstaged.hunks[0].lines.iter().any(|l| l.content == "line 2 changed"));

    // Stage a single added line of the first hunk (only the '+' line, keep the deletion).
    let h = &unstaged.hunks[0];
    let add_idx = h.lines.iter().position(|l| l.content == "line 2 changed").unwrap();
    let sel = vec![HunkSelection { hunk: 0, header: h.header.clone(), lines: Some(vec![add_idx]) }];
    ops::apply_lines(&repo, "f.txt", LineAction::Stage, &sel, 3).await.unwrap();
    let staged = read::working_diff(&repo, "f.txt", DiffSource::Staged, 3).await.unwrap();
    let all: Vec<_> = staged.hunks.iter().flat_map(|h| h.lines.iter()).map(|l| (l.kind, l.content.clone())).collect();
    assert!(all.contains(&(crate::git::diff::LineKind::Add, "line 2 changed".into())));
    assert!(!all.contains(&(crate::git::diff::LineKind::Del, "line 2".into())));

    // Unstage everything from the second hunk again.
    let staged = read::working_diff(&repo, "f.txt", DiffSource::Staged, 3).await.unwrap();
    let idx = staged.hunks.iter().position(|h| h.lines.iter().any(|l| l.content == "line 25 changed")).unwrap();
    let sel = vec![HunkSelection { hunk: idx, header: staged.hunks[idx].header.clone(), lines: None }];
    ops::apply_lines(&repo, "f.txt", LineAction::Unstage, &sel, 3).await.unwrap();
    let staged = read::working_diff(&repo, "f.txt", DiffSource::Staged, 3).await.unwrap();
    assert!(!staged.hunks.iter().flat_map(|h| h.lines.iter()).any(|l| l.content == "line 25 changed"));

    // Working tree file is untouched by index operations.
    assert_eq!(std::fs::read_to_string(repo.path.join("f.txt")).unwrap(), changed);

    // Stale selections are rejected.
    let stale = vec![HunkSelection { hunk: 0, header: "@@ -1,1 +1,1 @@".into(), lines: None }];
    assert!(ops::apply_lines(&repo, "f.txt", LineAction::Stage, &stale, 3).await.is_err());
}

#[tokio::test]
async fn cherry_pick_conflict_then_abort() {
    let (_d, repo) = setup().await;
    write(&repo, "c.txt", "base\n");
    commit_all(&repo, "base").await;
    ops::create_branch(&repo, "other", None, true).await.unwrap();
    write(&repo, "c.txt", "other side\n");
    commit_all(&repo, "other change").await;
    let other_sha = read::head_ref(&repo).await.oid.unwrap();
    ops::checkout(&repo, &CheckoutTarget::Branch { name: "main".into() }, false).await.unwrap();
    write(&repo, "c.txt", "main side\n");
    commit_all(&repo, "main change").await;
    let main_sha = read::head_ref(&repo).await.oid.unwrap();

    let res = ops::cherry_pick(&repo, &[other_sha], false).await.unwrap();
    assert!(res.conflicts, "{res:?}");
    let st = read::status(&repo, false).await.unwrap();
    assert_eq!(st.conflicted.len(), 1);
    assert_eq!(st.operation.as_ref().map(|o| o.kind), Some(OperationKind::CherryPick));

    // Continue must be refused while conflicts remain.
    assert!(ops::sequencer(&repo, SeqAction::Continue).await.is_err());
    ops::sequencer(&repo, SeqAction::Abort).await.unwrap();
    let st = read::status(&repo, false).await.unwrap();
    assert!(st.operation.is_none());
    assert!(st.conflicted.is_empty());
    assert_eq!(read::head_ref(&repo).await.oid.unwrap(), main_sha);
}

#[tokio::test]
async fn merge_conflict_resolve_with_theirs_and_continue() {
    let (_d, repo) = setup().await;
    write(&repo, "m.txt", "base\n");
    commit_all(&repo, "base").await;
    ops::create_branch(&repo, "topic", None, true).await.unwrap();
    write(&repo, "m.txt", "topic\n");
    commit_all(&repo, "topic").await;
    ops::checkout(&repo, &CheckoutTarget::Branch { name: "main".into() }, false).await.unwrap();
    write(&repo, "m.txt", "main\n");
    commit_all(&repo, "main").await;
    let res = ops::merge(&repo, "topic", MergeMode::Default).await.unwrap();
    assert!(res.conflicts);
    ops::resolve_conflicts(&repo, &["m.txt".into()], ops::ConflictResolution::Theirs).await.unwrap();
    let res = ops::sequencer(&repo, SeqAction::Continue).await.unwrap();
    assert!(res.ok, "{res:?}");
    assert_eq!(std::fs::read_to_string(repo.path.join("m.txt")).unwrap().replace("\r\n", "\n"), "topic\n");
    let detail = read::commit_detail(&repo, "HEAD").await.unwrap();
    assert_eq!(detail.parents.len(), 2);
}

#[tokio::test]
async fn stash_roundtrip_and_worktrees() {
    let (_d, repo) = setup().await;
    write(&repo, "s.txt", "v1\n");
    commit_all(&repo, "base").await;
    write(&repo, "s.txt", "v2\n");
    write(&repo, "new.txt", "untracked\n");
    ops::stash_push(&repo, &StashRequest { message: Some("wip test".into()), include_untracked: true, keep_index: false, paths: vec![] }).await.unwrap();
    let st = read::status(&repo, false).await.unwrap();
    assert!(st.unstaged.is_empty());
    let stashes = read::list_stashes(&repo).await.unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].message, "wip test");
    assert!(stashes[0].has_untracked);
    let untracked = read::stash_untracked_files(&repo, &stashes[0].hash).await.unwrap();
    assert_eq!(untracked.len(), 1);
    ops::stash_apply(&repo, 0, &stashes[0].hash, true, false).await.unwrap();
    assert!(read::list_stashes(&repo).await.unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(repo.path.join("s.txt")).unwrap(), "v2\n");
    commit_all(&repo, "second").await;

    // Worktree with a new branch, then refuse to check out the same branch twice.
    let wt_parent = tempfile::tempdir().unwrap();
    let wt_path = wt_parent.path().join("wt-feature");
    let req = WorktreeRequest { path: wt_path.display().to_string(), branch: None, new_branch: Some("wt-feature".into()), start: Some("HEAD".into()), detach: false };
    ops::worktree_add(&repo, &req).await.unwrap();
    let list = read::list_worktrees(&repo).await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|w| w.branch.as_deref() == Some("wt-feature") && w.dirty == Some(false)));
    let res = ops::checkout(&repo, &CheckoutTarget::Branch { name: "wt-feature".into() }, false).await;
    assert!(res.is_err());

    // Dirty worktree removal requires force.
    std::fs::write(wt_path.join("dirty.txt"), "x").unwrap();
    let wt = native_path(&wt_path.display().to_string());
    assert!(ops::worktree_remove(&repo, &wt, false).await.is_err());
    ops::worktree_remove(&repo, &wt, true).await.unwrap();
    assert_eq!(read::list_worktrees(&repo).await.unwrap().len(), 1);
}

#[tokio::test]
async fn discard_tracked_and_reset_modes() {
    let (_d, repo) = setup().await;
    write(&repo, "r.txt", "one\n");
    commit_all(&repo, "one").await;
    write(&repo, "r.txt", "two\n");
    commit_all(&repo, "two").await;
    write(&repo, "r.txt", "dirty\n");
    ops::discard(&repo, &["r.txt".into()], &[]).await.unwrap();
    assert_eq!(std::fs::read_to_string(repo.path.join("r.txt")).unwrap(), "two\n");

    // hard reset requires confirmation
    assert!(ops::reset(&repo, "HEAD~1", ops::ResetMode::Hard, false).await.is_err());
    ops::reset(&repo, "HEAD~1", ops::ResetMode::Soft, false).await.unwrap();
    let st = read::status(&repo, false).await.unwrap();
    assert_eq!(st.staged.len(), 1);
}

#[tokio::test]
async fn rejects_paths_outside_repo_and_option_injection() {
    let (_d, repo) = setup().await;
    assert!(ops::stage(&repo, &["../outside.txt".into()]).await.is_err());
    assert!(ops::create_branch(&repo, "--force", None, false).await.is_err());
    assert!(ops::remote_add(&repo, "origin", "--upload-pack=touch x").await.is_err());
    // A file literally named like an option is handled thanks to `--`.
    write(&repo, "-n", "x\n");
    ops::stage(&repo, &["-n".into()]).await.unwrap();
    let st = read::status(&repo, false).await.unwrap();
    assert!(st.staged.iter().any(|f| f.path == "-n"));
    let _ = Path::new("unused");
}
