//! Mutating local git operations. Callers must hold the per-repository write lock.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::diff::{self, HunkSelection, PatchDirection};
use super::read::{self, validate_path, validate_rev, DiffSource};
use super::runner::{GitOutput, RunOpts};
use crate::errors::{classify_git_failure, AppError, AppResult, ErrorCode};
use crate::state::Repo;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub ok: bool,
    pub message: String,
    pub conflicts: bool,
    pub details: Option<String>,
    /// Areas the UI should refresh: status, refs, graph, stashes, worktrees, remotes.
    pub changed: Vec<&'static str>,
}

impl OperationResult {
    pub fn ok(message: impl Into<String>, changed: &[&'static str]) -> Self {
        Self { ok: true, message: message.into(), changed: changed.to_vec(), ..Default::default() }
    }
    pub fn with_details(mut self, out: &GitOutput) -> Self {
        let text = format!("{}{}", out.text(), out.stderr);
        if !text.trim().is_empty() {
            self.details = Some(crate::redact::redact(text.trim()));
        }
        self
    }
}

pub const ALL: &[&str] = &["status", "refs", "graph", "stashes", "worktrees"];

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

pub async fn check_ref_name(repo: &Repo, kind: &str, name: &str) -> AppResult<()> {
    if name.trim().is_empty() || name.starts_with('-') || name.contains(char::is_whitespace) {
        return Err(AppError::invalid(format!("Nome inválido: \"{name}\".")));
    }
    let full = match kind {
        "branch" => format!("refs/heads/{name}"),
        "tag" => format!("refs/tags/{name}"),
        _ => format!("refs/remotes/{name}/x"),
    };
    let out = repo.git.run(&repo.path, &["check-ref-format", &full], RunOpts::read()).await?;
    if !out.ok() {
        return Err(AppError::invalid(format!("\"{name}\" não é um nome válido para o Git.")).recovery("Evite espaços, '..', '~', '^', ':', '?', '*', '[' e barras no início/fim."));
    }
    Ok(())
}

async fn has_conflicts(repo: &Repo) -> bool {
    match repo.git.run(&repo.path, &["diff", "--name-only", "--diff-filter=U", "-z"], RunOpts::read()).await {
        Ok(o) => o.stdout.iter().any(|b| *b != 0),
        Err(_) => false,
    }
}

/// Interprets the outcome of an operation that may stop on conflicts.
async fn conflict_aware(repo: &Repo, args: &[String], out: GitOutput, success: &str, changed: &[&'static str]) -> AppResult<OperationResult> {
    if out.ok() {
        return Ok(OperationResult::ok(success, changed).with_details(&out));
    }
    let combined = format!("{}\n{}", out.text(), out.stderr);
    let git_dir = read::git_dir(repo).await.ok();
    let in_progress = git_dir.as_deref().and_then(super::refs::detect_operation).is_some();
    if has_conflicts(repo).await || (in_progress && combined.to_lowercase().contains("conflict")) {
        return Ok(OperationResult {
            ok: false,
            conflicts: true,
            message: if in_progress {
                "A operação parou com conflitos. Resolva os arquivos e escolha Continuar, ou Abortar.".into()
            } else {
                "Há arquivos em conflito. Resolva-os (Nossa / Deles / editar e Marcar resolvido) e depois faça commit.".into()
            },
            details: Some(crate::redact::redact(combined.trim())),
            changed: changed.to_vec(),
        });
    }
    Err(classify_git_failure(args, &combined).repo(&repo.display))
}

async fn run_conflict_aware(repo: &Repo, args: Vec<String>, opts: RunOpts, success: &str, changed: &[&'static str]) -> AppResult<OperationResult> {
    let out = repo.git.run(&repo.path, &args, opts).await?;
    conflict_aware(repo, &args, out, success, changed).await
}

async fn run_simple(repo: &Repo, args: Vec<String>, success: &str, changed: &[&'static str]) -> AppResult<OperationResult> {
    let out = repo.git.run_ok(&repo.path, &args, RunOpts::write()).await.map_err(|e| e.repo(&repo.display))?;
    Ok(OperationResult::ok(success, changed).with_details(&out))
}

fn paths_args(base: &[&str], paths: &[String]) -> AppResult<Vec<String>> {
    let mut args = s(base);
    args.push("--".into());
    for p in paths {
        validate_path(p)?;
        args.push(p.clone());
    }
    Ok(args)
}

// ---------------------------------------------------------------- repository

pub async fn init(git: &super::runner::GitRunner, path: &Path, initial_branch: Option<&str>) -> AppResult<()> {
    std::fs::create_dir_all(path)?;
    let mut args = vec!["init".to_string()];
    if let Some(b) = initial_branch.filter(|b| !b.trim().is_empty()) {
        if b.starts_with('-') || b.contains(char::is_whitespace) {
            return Err(AppError::invalid("Nome de branch inicial inválido."));
        }
        args.push(format!("--initial-branch={b}"));
    }
    git.run_ok(path, &args, RunOpts::write()).await?;
    Ok(())
}

// ---------------------------------------------------------------- index

pub async fn stage(repo: &Repo, paths: &[String]) -> AppResult<OperationResult> {
    if paths.is_empty() {
        return run_simple(repo, s(&["add", "-A"]), "Todas as alterações foram preparadas.", &["status"]).await;
    }
    let mut total = 0;
    for chunk in paths.chunks(200) {
        run_simple(repo, paths_args(&["add", "-A"], chunk)?, "", &[]).await?;
        total += chunk.len();
    }
    Ok(OperationResult::ok(format!("{total} arquivo(s) preparado(s)."), &["status"]))
}

pub async fn unstage(repo: &Repo, paths: &[String]) -> AppResult<OperationResult> {
    let unborn = read::head_ref(repo).await.oid.is_none();
    if paths.is_empty() {
        let args = if unborn { s(&["rm", "--cached", "-r", "-q", "--", "."]) } else { s(&["reset", "-q"]) };
        return run_simple(repo, args, "Todas as alterações foram removidas do stage.", &["status"]).await;
    }
    for chunk in paths.chunks(200) {
        let args = if unborn { paths_args(&["rm", "--cached", "-r", "-q"], chunk)? } else { paths_args(&["restore", "--staged"], chunk)? };
        run_simple(repo, args, "", &[]).await?;
    }
    Ok(OperationResult::ok(format!("{} arquivo(s) removido(s) do stage.", paths.len()), &["status"]))
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineAction {
    Stage,
    Unstage,
    Discard,
}

pub async fn apply_lines(repo: &Repo, path: &str, action: LineAction, selections: &[HunkSelection], ctx: u32) -> AppResult<OperationResult> {
    let source = match action {
        LineAction::Stage | LineAction::Discard => DiffSource::Unstaged,
        LineAction::Unstage => DiffSource::Staged,
    };
    let raw = read::raw_working_diff(repo, path, source, ctx).await?;
    let files = diff::split_files(&raw);
    let Some(file) = files.first() else {
        return Err(AppError::new(ErrorCode::StaleState, "O arquivo não tem mais alterações; atualize a visualização."));
    };
    let parsed = diff::parse_raw(file);
    if parsed.binary {
        return Err(AppError::invalid("Arquivos binários só podem ser preparados por inteiro."));
    }
    let dir = if action == LineAction::Stage { PatchDirection::Forward } else { PatchDirection::Reverse };
    let patch = diff::build_partial_patch(&parsed, selections, dir).map_err(|m| AppError::new(ErrorCode::StaleState, m))?;
    let Some(patch) = patch else {
        return Ok(OperationResult::ok("Nenhuma linha alterada selecionada.", &[]));
    };
    let mut args = s(&["apply", "--whitespace=nowarn"]);
    if ctx == 0 {
        args.push("--unidiff-zero".into());
    }
    match action {
        LineAction::Stage => args.push("--cached".into()),
        LineAction::Unstage => {
            args.push("--cached".into());
            args.push("--reverse".into());
        }
        LineAction::Discard => args.push("--reverse".into()),
    }
    args.push("-".into());
    let out = repo.git.run(&repo.path, &args, RunOpts::write().stdin(patch)).await?;
    if !out.ok() {
        return Err(AppError::new(ErrorCode::GitFailed, "O Git não conseguiu aplicar a seleção de linhas.")
            .details(out.stderr)
            .recovery("Atualize o diff e tente novamente, ou prepare o hunk/arquivo inteiro."));
    }
    let msg = match action {
        LineAction::Stage => "Linhas preparadas.",
        LineAction::Unstage => "Linhas removidas do stage.",
        LineAction::Discard => "Linhas descartadas.",
    };
    Ok(OperationResult::ok(msg, &["status"]))
}

/// Discards working tree changes. Tracked files are restored from the index; untracked
/// files are moved to the Recycle Bin (recoverable) instead of being deleted.
pub async fn discard(repo: &Repo, tracked: &[String], untracked: &[String]) -> AppResult<OperationResult> {
    for chunk in tracked.chunks(200) {
        run_simple(repo, paths_args(&["restore", "--worktree"], chunk)?, "", &[]).await?;
    }
    let mut trashed = 0;
    let mut failures = Vec::new();
    for p in untracked {
        validate_path(p)?;
        let full = repo.path.join(p);
        if !full.exists() {
            continue;
        }
        match trash::delete(&full) {
            Ok(()) => trashed += 1,
            Err(e) => failures.push(format!("{p}: {e}")),
        }
    }
    if !failures.is_empty() {
        return Err(AppError::new(ErrorCode::Io, "Alguns arquivos não puderam ser movidos para a Lixeira.").details(failures.join("\n")));
    }
    let mut msg = format!("{} arquivo(s) restaurado(s)", tracked.len());
    if trashed > 0 {
        msg.push_str(&format!(", {trashed} arquivo(s) não rastreado(s) movido(s) para a Lixeira"));
    }
    msg.push('.');
    Ok(OperationResult::ok(msg, &["status"]))
}

// ---------------------------------------------------------------- commit

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    pub message: String,
    #[serde(default)]
    pub amend: bool,
    #[serde(default)]
    pub sign_off: bool,
    /// None = respect commit.gpgsign; Some(true) = -S; Some(false) = --no-gpg-sign.
    pub gpg_sign: Option<bool>,
    #[serde(default)]
    pub no_verify: bool,
    #[serde(default)]
    pub allow_empty: bool,
}

pub async fn commit(repo: &Repo, req: &CommitRequest) -> AppResult<OperationResult> {
    if req.message.trim().is_empty() {
        return Err(AppError::invalid("A mensagem de commit não pode ficar vazia."));
    }
    let mut args = s(&["commit", "-F", "-", "--cleanup=strip"]);
    if req.amend {
        args.push("--amend".into());
    }
    if req.sign_off {
        args.push("--signoff".into());
    }
    match req.gpg_sign {
        Some(true) => args.push("-S".into()),
        Some(false) => args.push("--no-gpg-sign".into()),
        None => {}
    }
    if req.no_verify {
        args.push("--no-verify".into());
    }
    if req.allow_empty {
        args.push("--allow-empty".into());
    }
    let out = repo.git.run(&repo.path, &args, RunOpts::write().stdin(req.message.clone().into_bytes())).await?;
    if !out.ok() {
        let text = format!("{}\n{}", out.text(), out.stderr);
        let lower = text.to_lowercase();
        if lower.contains("please tell me who you are") || lower.contains("unable to auto-detect email") {
            return Err(AppError::new(ErrorCode::GitFailed, "Identidade Git não configurada (user.name / user.email).")
                .details(text)
                .recovery("Configure nome e e-mail em Configurações > Identidade Git."));
        }
        if lower.contains("nothing to commit") || lower.contains("no changes added to commit") {
            return Err(AppError::new(ErrorCode::InvalidInput, "Não há alterações preparadas para commit.").details(text));
        }
        if lower.contains("gpg failed") || lower.contains("failed to sign") {
            return Err(AppError::new(ErrorCode::GitFailed, "Falha ao assinar o commit (GPG/SSH).").details(text)
                .recovery("Verifique a configuração de assinatura (user.signingkey, gpg.program) ou desative a assinatura neste commit."));
        }
        if lower.contains("hook") {
            return Err(AppError::new(ErrorCode::GitFailed, "Um hook do repositório bloqueou o commit.").details(text)
                .recovery("Corrija o problema apontado pelo hook ou, se souber o que está fazendo, use a opção para ignorar hooks."));
        }
        return Err(classify_git_failure(&args, &text).repo(&repo.display));
    }
    let head = read::head_ref(repo).await;
    let short = head.oid.as_deref().map(|h| &h[..h.len().min(8)]).unwrap_or("");
    Ok(OperationResult::ok(if req.amend { format!("Commit {short} corrigido (amend).") } else { format!("Commit {short} criado.") }, &["status", "refs", "graph"])
        .with_details(&out))
}

pub async fn last_commit_message(repo: &Repo) -> AppResult<String> {
    Ok(repo.git.read(&repo.path, &["log", "-1", "--format=%B"]).await?.trim_end().to_string())
}

pub async fn set_identity(repo: &Repo, global: bool, name: &str, email: &str) -> AppResult<()> {
    let scope = if global { "--global" } else { "--local" };
    for (k, v) in [("user.name", name), ("user.email", email)] {
        if v.trim().is_empty() {
            repo.git.run(&repo.path, &["config", scope, "--unset", k], RunOpts::write()).await?;
        } else {
            repo.git.run_ok(&repo.path, &["config", scope, k, v.trim()], RunOpts::write()).await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- branches

pub async fn create_branch(repo: &Repo, name: &str, start: Option<&str>, checkout: bool) -> AppResult<OperationResult> {
    check_ref_name(repo, "branch", name).await?;
    let mut args = if checkout { s(&["switch", "-c", name]) } else { s(&["branch", name]) };
    if let Some(st) = start {
        validate_rev(st)?;
        args.push(st.to_string());
    }
    run_simple(repo, args, &format!("Branch '{name}' criada."), &["refs", "graph", "status"]).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CheckoutTarget {
    Branch { name: String },
    Remote { remote_branch: String, local_name: String },
    Detached { rev: String },
}

pub async fn checkout(repo: &Repo, target: &CheckoutTarget, auto_stash: bool) -> AppResult<OperationResult> {
    let args = match target {
        CheckoutTarget::Branch { name } => {
            check_ref_name(repo, "branch", name).await?;
            s(&["switch", name])
        }
        CheckoutTarget::Remote { remote_branch, local_name } => {
            check_ref_name(repo, "branch", local_name).await?;
            validate_rev(remote_branch)?;
            s(&["switch", "-c", local_name, "--track", remote_branch])
        }
        CheckoutTarget::Detached { rev } => {
            validate_rev(rev)?;
            s(&["switch", "--detach", rev])
        }
    };
    let mut stashed = false;
    if auto_stash {
        let st = read::status(repo, false).await?;
        if !(st.staged.is_empty() && st.unstaged.is_empty()) {
            run_simple(repo, s(&["stash", "push", "--include-untracked", "-m", "ForgeGraph: auto-stash antes do checkout"]), "", &[]).await?;
            stashed = true;
        }
    }
    let out = repo.git.run(&repo.path, &args, RunOpts::write()).await?;
    if !out.ok() {
        if stashed {
            // Put the changes back where they were.
            let _ = repo.git.run(&repo.path, &["stash", "pop", "--index"], RunOpts::write()).await;
        }
        let text = format!("{}\n{}", out.text(), out.stderr);
        if text.contains("is already used by worktree") || text.contains("is already checked out at") {
            return Err(AppError::new(ErrorCode::GitFailed, "Esta branch já está em uso por outra worktree.")
                .details(text)
                .recovery("Abra a worktree correspondente ou crie outra branch a partir dela."));
        }
        return Err(classify_git_failure(&args, &text).repo(&repo.display));
    }
    let mut res = OperationResult::ok(
        match target {
            CheckoutTarget::Detached { rev } => format!("HEAD destacado em {}. Commits feitos aqui não pertencem a nenhuma branch.", &rev[..rev.len().min(10)]),
            CheckoutTarget::Branch { name } => format!("Agora em '{name}'."),
            CheckoutTarget::Remote { local_name, .. } => format!("Branch local '{local_name}' criada e ativa."),
        },
        ALL,
    );
    if stashed {
        let pop = repo.git.run(&repo.path, &["stash", "pop"], RunOpts::write()).await?;
        if !pop.ok() {
            res.conflicts = has_conflicts(repo).await;
            res.message.push_str(" As alterações locais ficaram no stash porque houve conflito ao reaplicá-las.");
            res.details = Some(pop.stderr);
        } else {
            res.message.push_str(" Alterações locais reaplicadas.");
        }
    }
    Ok(res)
}

pub async fn rename_branch(repo: &Repo, old: &str, new: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "branch", old).await?;
    check_ref_name(repo, "branch", new).await?;
    run_simple(repo, s(&["branch", "-m", old, new]), &format!("Branch renomeada para '{new}'."), &["refs", "graph", "status", "worktrees"]).await
}

pub async fn delete_branch(repo: &Repo, name: &str, force: bool) -> AppResult<OperationResult> {
    check_ref_name(repo, "branch", name).await?;
    let out = repo.git.run(&repo.path, &["branch", if force { "-D" } else { "-d" }, name], RunOpts::write()).await?;
    if !out.ok() {
        if out.stderr.contains("not fully merged") {
            return Err(AppError::new(ErrorCode::InvalidInput, format!("A branch '{name}' tem commits que não estão em nenhuma outra branch."))
                .details(out.stderr)
                .recovery("Faça merge antes, ou confirme a exclusão forçada (os commits só ficarão acessíveis pelo reflog)."));
        }
        return Err(classify_git_failure(&s(&["branch", "-d"]), &out.stderr));
    }
    Ok(OperationResult::ok(format!("Branch '{name}' excluída."), &["refs", "graph"]))
}

pub async fn set_upstream(repo: &Repo, branch: &str, upstream: Option<&str>) -> AppResult<OperationResult> {
    check_ref_name(repo, "branch", branch).await?;
    match upstream {
        Some(u) => {
            validate_rev(u)?;
            run_simple(repo, vec!["branch".into(), format!("--set-upstream-to={u}"), branch.into()], &format!("'{branch}' agora rastreia '{u}'."), &["refs", "status"]).await
        }
        None => run_simple(repo, s(&["branch", "--unset-upstream", branch]), "Upstream removido.", &["refs", "status"]).await,
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Default,
    NoFf,
    FfOnly,
    Squash,
}

pub async fn merge(repo: &Repo, target: &str, mode: MergeMode) -> AppResult<OperationResult> {
    validate_rev(target)?;
    let mut args = s(&["merge", "--no-edit"]);
    match mode {
        MergeMode::NoFf => args.push("--no-ff".into()),
        MergeMode::FfOnly => args.push("--ff-only".into()),
        MergeMode::Squash => args.push("--squash".into()),
        MergeMode::Default => {}
    }
    args.push(target.into());
    let msg = if mode == MergeMode::Squash { format!("Alterações de '{target}' combinadas no index (squash). Revise e faça commit.") } else { format!("Merge de '{target}' concluído.") };
    run_conflict_aware(repo, args, RunOpts::write(), &msg, ALL).await
}

pub async fn rebase(repo: &Repo, onto: &str) -> AppResult<OperationResult> {
    validate_rev(onto)?;
    run_conflict_aware(repo, s(&["rebase", onto]), RunOpts::write(), &format!("Rebase sobre '{onto}' concluído."), ALL).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub action: String,
    pub hash: String,
}

/// Interactive rebase driven by a todo list prepared in the UI (pick/squash/fixup/drop/reorder).
pub async fn rebase_interactive(repo: &Repo, base: &str, todo: &[TodoItem]) -> AppResult<OperationResult> {
    validate_rev(base)?;
    if todo.is_empty() {
        return Err(AppError::invalid("Lista de commits vazia."));
    }
    let mut text = String::new();
    for (i, item) in todo.iter().enumerate() {
        validate_rev(&item.hash)?;
        let action = match item.action.as_str() {
            "pick" | "squash" | "fixup" | "drop" => item.action.as_str(),
            _ => return Err(AppError::invalid(format!("Ação de rebase não suportada: {}", item.action))),
        };
        if i == 0 && (action == "squash" || action == "fixup") {
            return Err(AppError::invalid("O primeiro commit não pode ser squash/fixup."));
        }
        text.push_str(&format!("{action} {}\n", item.hash));
    }
    let dir = std::env::temp_dir().join("forgegraph");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("rebase-todo-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&file, text)?;
    let fwd = file.display().to_string().replace('\\', "/");
    if fwd.contains('\'') {
        return Err(AppError::internal("Caminho temporário inválido."));
    }
    let opts = RunOpts::write().env("GIT_SEQUENCE_EDITOR", format!("cp '{fwd}'"));
    let res = run_conflict_aware(repo, s(&["rebase", "-i", base]), opts, "Rebase interativo concluído.", ALL).await;
    let _ = std::fs::remove_file(&file);
    res
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeqAction {
    Continue,
    Abort,
    Skip,
}

pub async fn sequencer(repo: &Repo, action: SeqAction) -> AppResult<OperationResult> {
    let gd = read::git_dir(repo).await?;
    let Some(op) = super::refs::detect_operation(&gd) else {
        return Err(AppError::new(ErrorCode::StaleState, "Não há merge, rebase, cherry-pick ou revert em andamento."));
    };
    use super::refs::OperationKind as K;
    let verb = match op.kind {
        K::Merge => "merge",
        K::Rebase => "rebase",
        K::CherryPick => "cherry-pick",
        K::Revert => "revert",
        K::ApplyMailbox => "am",
        K::Bisect => return Err(AppError::invalid("Bisect não é controlado por esta ação.")),
    };
    if op.kind == K::Merge && action == SeqAction::Skip {
        return Err(AppError::invalid("Merge não suporta Skip."));
    }
    if action == SeqAction::Continue && has_conflicts(repo).await {
        return Err(AppError::new(ErrorCode::Conflict, "Ainda existem arquivos em conflito.").recovery("Resolva e marque todos os arquivos como resolvidos antes de continuar."));
    }
    let flag = match action {
        SeqAction::Continue => "--continue",
        SeqAction::Abort => "--abort",
        SeqAction::Skip => "--skip",
    };
    let msg = match action {
        SeqAction::Continue => format!("{verb}: continuado."),
        SeqAction::Abort => format!("{verb}: abortado; o repositório voltou ao estado anterior."),
        SeqAction::Skip => format!("{verb}: commit atual ignorado."),
    };
    run_conflict_aware(repo, vec![verb.into(), flag.into()], RunOpts::write(), &msg, ALL).await
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    Ours,
    Theirs,
    MarkResolved,
}

pub async fn resolve_conflicts(repo: &Repo, paths: &[String], how: ConflictResolution) -> AppResult<OperationResult> {
    for p in paths {
        validate_path(p)?;
        match how {
            ConflictResolution::Ours | ConflictResolution::Theirs => {
                let side = if how == ConflictResolution::Ours { "--ours" } else { "--theirs" };
                let out = repo.git.run(&repo.path, &["checkout", side, "--", p], RunOpts::write()).await?;
                if !out.ok() {
                    // That side deleted the file: resolving to it means removing it.
                    if out.stderr.contains("does not have") {
                        repo.git.run_ok(&repo.path, &["rm", "-q", "--", p], RunOpts::write()).await?;
                        continue;
                    }
                    return Err(classify_git_failure(&s(&["checkout"]), &out.stderr));
                }
                repo.git.run_ok(&repo.path, &["add", "--", p], RunOpts::write()).await?;
            }
            ConflictResolution::MarkResolved => {
                if repo.path.join(p).exists() {
                    repo.git.run_ok(&repo.path, &["add", "--", p], RunOpts::write()).await?;
                } else {
                    repo.git.run_ok(&repo.path, &["rm", "-q", "--", p], RunOpts::write()).await?;
                }
            }
        }
    }
    Ok(OperationResult::ok(format!("{} arquivo(s) marcados como resolvidos.", paths.len()), &["status"]))
}

async fn is_merge_commit(repo: &Repo, sha: &str) -> bool {
    match repo.git.run(&repo.path, &["rev-list", "--parents", "-n", "1", sha], RunOpts::read()).await {
        Ok(o) if o.ok() => o.text().split_whitespace().count() > 2,
        _ => false,
    }
}

pub async fn cherry_pick(repo: &Repo, shas: &[String], no_commit: bool) -> AppResult<OperationResult> {
    if shas.is_empty() {
        return Err(AppError::invalid("Selecione ao menos um commit."));
    }
    let mut args = s(&["cherry-pick"]);
    if no_commit {
        args.push("--no-commit".into());
    }
    let mut any_merge = false;
    for sha in shas {
        validate_rev(sha)?;
        any_merge |= is_merge_commit(repo, sha).await;
    }
    if any_merge {
        args.push("-m".into());
        args.push("1".into());
    }
    args.extend(shas.iter().cloned());
    run_conflict_aware(repo, args, RunOpts::write(), &format!("{} commit(s) aplicados via cherry-pick.", shas.len()), ALL).await
}

pub async fn revert(repo: &Repo, sha: &str, no_commit: bool) -> AppResult<OperationResult> {
    validate_rev(sha)?;
    let mut args = s(&["revert", "--no-edit"]);
    if no_commit {
        args.push("--no-commit".into());
    }
    if is_merge_commit(repo, sha).await {
        args.push("-m".into());
        args.push("1".into());
    }
    args.push(sha.into());
    run_conflict_aware(repo, args, RunOpts::write(), "Commit revertido com um novo commit.", ALL).await
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
    Keep,
}

pub async fn reset(repo: &Repo, rev: &str, mode: ResetMode, confirmed: bool) -> AppResult<OperationResult> {
    validate_rev(rev)?;
    if mode == ResetMode::Hard && !confirmed {
        return Err(AppError::invalid("Reset hard exige confirmação explícita."));
    }
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
        ResetMode::Keep => "--keep",
    };
    run_simple(repo, s(&["reset", "-q", flag, rev]), &format!("Branch redefinida para {} ({}).", &rev[..rev.len().min(10)], &flag[2..]), ALL).await
}

// ---------------------------------------------------------------- tags

pub async fn create_tag(repo: &Repo, name: &str, target: &str, message: Option<&str>) -> AppResult<OperationResult> {
    check_ref_name(repo, "tag", name).await?;
    validate_rev(target)?;
    match message.filter(|m| !m.trim().is_empty()) {
        Some(m) => {
            let args = s(&["tag", "-a", "-F", "-", name, target]);
            repo.git.run_ok(&repo.path, &args, RunOpts::write().stdin(m.as_bytes().to_vec())).await?;
        }
        None => {
            repo.git.run_ok(&repo.path, &["tag", name, target], RunOpts::write()).await?;
        }
    }
    Ok(OperationResult::ok(format!("Tag '{name}' criada."), &["refs", "graph"]))
}

pub async fn delete_tag(repo: &Repo, name: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "tag", name).await?;
    run_simple(repo, s(&["tag", "-d", name]), &format!("Tag local '{name}' excluída."), &["refs", "graph"]).await
}

/// Renames a tag by recreating it on the same object (keeps annotation) and deleting the old name.
pub async fn rename_tag(repo: &Repo, old: &str, new: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "tag", old).await?;
    check_ref_name(repo, "tag", new).await?;
    let obj = repo.git.read(&repo.path, &["rev-parse", "--verify", &format!("refs/tags/{old}")]).await?;
    let obj = obj.trim().to_string();
    repo.git.run_ok(&repo.path, &["tag", new, &obj], RunOpts::write()).await?;
    repo.git.run_ok(&repo.path, &["tag", "-d", old], RunOpts::write()).await?;
    Ok(OperationResult::ok(format!("Tag renomeada localmente para '{new}'. Remotes não foram alterados."), &["refs", "graph"]))
}

// ---------------------------------------------------------------- stash

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashRequest {
    pub message: Option<String>,
    #[serde(default)]
    pub include_untracked: bool,
    #[serde(default)]
    pub keep_index: bool,
    #[serde(default)]
    pub paths: Vec<String>,
}

pub async fn stash_push(repo: &Repo, req: &StashRequest) -> AppResult<OperationResult> {
    let mut args = s(&["stash", "push"]);
    if req.include_untracked {
        args.push("--include-untracked".into());
    }
    if req.keep_index {
        args.push("--keep-index".into());
    }
    if let Some(m) = req.message.as_deref().filter(|m| !m.trim().is_empty()) {
        args.push("-m".into());
        args.push(m.trim().into());
    }
    if !req.paths.is_empty() {
        args.push("--".into());
        for p in &req.paths {
            validate_path(p)?;
            args.push(p.clone());
        }
    }
    let out = repo.git.run_ok(&repo.path, &args, RunOpts::write()).await?;
    if out.text().contains("No local changes to save") {
        return Err(AppError::invalid("Não há alterações locais para guardar no stash."));
    }
    Ok(OperationResult::ok("Alterações guardadas no stash.", &["status", "stashes", "graph"]))
}

async fn verify_stash(repo: &Repo, index: u32, hash: &str) -> AppResult<String> {
    let r = format!("stash@{{{index}}}");
    let cur = repo.git.read(&repo.path, &["rev-parse", "--verify", &r]).await.map_err(|_| AppError::new(ErrorCode::StaleState, "Stash não encontrado; atualize a lista."))?;
    if cur.trim() != hash {
        return Err(AppError::new(ErrorCode::StaleState, "A lista de stashes mudou; atualize e tente novamente."));
    }
    Ok(r)
}

pub async fn stash_apply(repo: &Repo, index: u32, hash: &str, pop: bool, restore_index: bool) -> AppResult<OperationResult> {
    let r = verify_stash(repo, index, hash).await?;
    let mut args = s(&["stash", if pop { "pop" } else { "apply" }]);
    if restore_index {
        args.push("--index".into());
    }
    args.push(r);
    let msg = if pop { "Stash aplicado e removido." } else { "Stash aplicado." };
    let mut res = run_conflict_aware(repo, args, RunOpts::write(), msg, &["status", "stashes", "graph"]).await?;
    if res.conflicts && pop {
        res.message.push_str(" O stash foi mantido na lista por causa do conflito.");
    }
    Ok(res)
}

pub async fn stash_drop(repo: &Repo, index: u32, hash: &str) -> AppResult<OperationResult> {
    let r = verify_stash(repo, index, hash).await?;
    run_simple(repo, s(&["stash", "drop", &r]), "Stash descartado.", &["stashes", "graph"]).await
}

pub async fn stash_clear(repo: &Repo) -> AppResult<OperationResult> {
    run_simple(repo, s(&["stash", "clear"]), "Todos os stashes foram removidos.", &["stashes", "graph"]).await
}

pub async fn stash_branch(repo: &Repo, index: u32, hash: &str, name: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "branch", name).await?;
    let r = verify_stash(repo, index, hash).await?;
    run_conflict_aware(repo, s(&["stash", "branch", name, &r]), RunOpts::write(), &format!("Branch '{name}' criada a partir do stash."), ALL).await
}

// ---------------------------------------------------------------- worktrees

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRequest {
    pub path: String,
    /// Existing branch to check out in the new worktree.
    pub branch: Option<String>,
    /// Create this new branch (from `start`).
    pub new_branch: Option<String>,
    pub start: Option<String>,
    #[serde(default)]
    pub detach: bool,
}

pub async fn worktree_add(repo: &Repo, req: &WorktreeRequest) -> AppResult<OperationResult> {
    let p = Path::new(&req.path);
    if !p.is_absolute() {
        return Err(AppError::invalid("Escolha uma pasta de destino (caminho absoluto)."));
    }
    if p.exists() && std::fs::read_dir(p).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(AppError::invalid("A pasta de destino já existe e não está vazia."));
    }
    let mut args = s(&["worktree", "add"]);
    if let Some(nb) = req.new_branch.as_deref().filter(|b| !b.is_empty()) {
        check_ref_name(repo, "branch", nb).await?;
        args.push("-b".into());
        args.push(nb.into());
    } else if req.detach {
        args.push("--detach".into());
    }
    args.push("--".into());
    args.push(req.path.clone());
    if let Some(b) = req.branch.as_deref().filter(|b| !b.is_empty()) {
        validate_rev(b)?;
        args.push(b.into());
    } else if let Some(st) = req.start.as_deref().filter(|b| !b.is_empty()) {
        validate_rev(st)?;
        args.push(st.into());
    }
    let out = repo.git.run(&repo.path, &args, RunOpts::write()).await?;
    if !out.ok() {
        if out.stderr.contains("is already checked out") || out.stderr.contains("is already used by worktree") {
            return Err(AppError::new(ErrorCode::InvalidInput, "Esta branch já está em uso em outra worktree. O Git não permite a mesma branch em duas worktrees.")
                .details(out.stderr)
                .recovery("Crie uma nova branch para a worktree ou use HEAD destacado."));
        }
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(format!("Worktree criada em {}.", req.path), &["worktrees", "refs", "graph"]))
}

pub async fn worktree_remove(repo: &Repo, path: &str, force: bool) -> AppResult<OperationResult> {
    let mut args = s(&["worktree", "remove"]);
    if force {
        args.push("--force".into());
    }
    args.push("--".into());
    args.push(path.into());
    let out = repo.git.run(&repo.path, &args, RunOpts::write()).await?;
    if !out.ok() {
        if out.stderr.contains("contains modified or untracked files") || out.stderr.contains("is dirty") {
            return Err(AppError::new(ErrorCode::DirtyWorktree, "A worktree tem alterações não commitadas.")
                .details(out.stderr)
                .recovery("Faça commit/stash nela, ou confirme a remoção forçada (as alterações serão perdidas)."));
        }
        if out.stderr.contains("locked") {
            return Err(AppError::new(ErrorCode::InvalidInput, "A worktree está bloqueada (locked).").details(out.stderr).recovery("Desbloqueie a worktree antes de removê-la."));
        }
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok("Worktree removida.", &["worktrees", "refs"]))
}

pub async fn worktree_prune(repo: &Repo) -> AppResult<OperationResult> {
    run_simple(repo, s(&["worktree", "prune", "-v"]), "Registros de worktrees inexistentes foram limpos.", &["worktrees"]).await
}

pub async fn worktree_lock(repo: &Repo, path: &str, lock: bool, reason: Option<&str>) -> AppResult<OperationResult> {
    let mut args = s(&["worktree", if lock { "lock" } else { "unlock" }]);
    if lock {
        if let Some(r) = reason.filter(|r| !r.trim().is_empty()) {
            args.push("--reason".into());
            args.push(r.trim().into());
        }
    }
    args.push(path.into());
    run_simple(repo, args, if lock { "Worktree bloqueada." } else { "Worktree desbloqueada." }, &["worktrees"]).await
}

// ---------------------------------------------------------------- remotes (local config)

pub fn validate_remote_url(url: &str) -> AppResult<()> {
    let u = url.trim();
    if u.is_empty() || u.starts_with('-') || u.contains(char::is_whitespace) {
        return Err(AppError::invalid("URL de remote inválida."));
    }
    if let Ok(parsed) = url::Url::parse(u) {
        if parsed.password().is_some() {
            return Err(AppError::invalid("Não inclua senha/token na URL do remote.")
                .recovery("Use o Git Credential Manager, SSH ou uma conta conectada para autenticar."));
        }
        if matches!(parsed.scheme(), "ext" | "fd") {
            return Err(AppError::invalid("Protocolo de remote não permitido."));
        }
    }
    Ok(())
}

pub async fn remote_add(repo: &Repo, name: &str, url: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", name).await?;
    validate_remote_url(url)?;
    run_simple(repo, s(&["remote", "add", name, url.trim()]), &format!("Remote '{name}' adicionado."), &["remotes", "refs"]).await
}

pub async fn remote_set_url(repo: &Repo, name: &str, url: &str, push: bool) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", name).await?;
    validate_remote_url(url)?;
    let mut args = s(&["remote", "set-url"]);
    if push {
        args.push("--push".into());
    }
    args.push(name.into());
    args.push(url.trim().into());
    run_simple(repo, args, "URL do remote atualizada.", &["remotes"]).await
}

pub async fn remote_rename(repo: &Repo, old: &str, new: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", old).await?;
    check_ref_name(repo, "remote", new).await?;
    run_simple(repo, s(&["remote", "rename", old, new]), "Remote renomeado.", &["remotes", "refs", "graph"]).await
}

pub async fn remote_remove(repo: &Repo, name: &str) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", name).await?;
    run_simple(repo, s(&["remote", "remove", name]), &format!("Remote '{name}' removido."), &["remotes", "refs", "graph"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_url_validation() {
        assert!(validate_remote_url("git@github.com:a/b.git").is_ok());
        assert!(validate_remote_url("https://github.com/a/b.git").is_ok());
        assert!(validate_remote_url("https://u:tok@github.com/a/b.git").is_err());
        assert!(validate_remote_url("--upload-pack=evil").is_err());
        assert!(validate_remote_url("ext::sh -c x").is_err());
    }
}
