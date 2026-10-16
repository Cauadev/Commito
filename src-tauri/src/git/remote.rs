//! Network operations: clone, fetch, pull, push and remote ref management.
//!
//! Authentication: by default git uses its own chain (Git Credential Manager for HTTPS,
//! ssh-agent / ~/.ssh/config for SSH). If the user linked a connected account to a repo,
//! HTTPS remotes on that account's host use the askpass bridge (token read from the OS vault).

use std::path::Path;

use serde::Deserialize;

use super::ops::{check_ref_name, validate_remote_url, OperationResult, ALL};
use super::read::{self, validate_rev};
use super::runner::{GitRunner, RunOpts};
use crate::credentials;
use crate::errors::{classify_git_failure, AppError, AppResult, ErrorCode};
use crate::events::ProgressSink;
use crate::settings::ProviderAccount;
use crate::state::Repo;

/// Returns the account to use for `url` if it is HTTPS on the account's host.
pub fn account_for_url<'a>(account: Option<&'a ProviderAccount>, url: &str) -> Option<&'a ProviderAccount> {
    let account = account?;
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_lowercase();
    (host == crate::providers::git_host(account).to_lowercase()).then_some(account)
}

fn net_opts(progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<(RunOpts, Vec<String>)> {
    let opts = RunOpts::network(progress);
    match account {
        Some(acc) => Ok((credentials::with_askpass(opts, acc)?, credentials::askpass_args())),
        None => Ok((opts, vec![])),
    }
}

async fn remote_url(repo: &Repo, remote: &str) -> Option<String> {
    let o = repo.git.run(&repo.path, &["remote", "get-url", remote], RunOpts::read()).await.ok()?;
    o.ok().then(|| o.text().trim().to_string())
}

async fn run_net(repo: &Repo, remote: &str, mut args: Vec<String>, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<super::runner::GitOutput> {
    let url = remote_url(repo, remote).await.unwrap_or_default();
    let acc = account_for_url(account, &url);
    let (opts, mut prefix) = net_opts(progress, acc)?;
    prefix.append(&mut args);
    repo.git.run(&repo.path, &prefix, opts).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub url: String,
    pub parent_dir: String,
    pub dir_name: String,
    pub branch: Option<String>,
    pub depth: Option<u32>,
    #[serde(default)]
    pub recurse_submodules: bool,
}

pub async fn clone(git: &GitRunner, req: &CloneRequest, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<String> {
    validate_remote_url(&req.url)?;
    let name = req.dir_name.trim();
    if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) || name == "." || name == ".." {
        return Err(AppError::invalid("Nome de pasta inválido."));
    }
    let parent = Path::new(&req.parent_dir);
    if !parent.is_absolute() {
        return Err(AppError::invalid("Escolha uma pasta de destino."));
    }
    std::fs::create_dir_all(parent)?;
    let dest = parent.join(name);
    if dest.exists() && std::fs::read_dir(&dest).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(AppError::invalid(format!("A pasta {} já existe e não está vazia.", dest.display())));
    }
    let acc = account_for_url(account, &req.url);
    let (opts, mut args) = net_opts(progress, acc)?;
    args.extend(["clone".to_string(), "--progress".to_string()]);
    if let Some(b) = req.branch.as_deref().filter(|b| !b.trim().is_empty()) {
        validate_rev(b)?;
        args.push("--branch".into());
        args.push(b.trim().into());
    }
    if let Some(d) = req.depth.filter(|d| *d > 0) {
        args.push(format!("--depth={d}"));
    }
    if req.recurse_submodules {
        args.push("--recurse-submodules".into());
    }
    args.push("--".into());
    args.push(req.url.trim().into());
    args.push(dest.display().to_string());
    let out = git.run(parent, &args, opts).await?;
    if !out.ok() {
        // git removes the partially cloned directory itself on failure.
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(dest.display().to_string())
}

pub async fn fetch(repo: &Repo, remote: Option<&str>, prune: bool, tags: bool, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    let remotes: Vec<String> = match remote {
        Some(r) => {
            check_ref_name(repo, "remote", r).await?;
            vec![r.to_string()]
        }
        None => read::list_remotes(repo).await?.into_iter().map(|r| r.name).collect(),
    };
    if remotes.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "Este repositório não tem remotes configurados.").recovery("Adicione um remote na seção Remotes."));
    }
    let mut failures = Vec::new();
    for r in &remotes {
        let mut args = vec!["fetch".to_string(), "--progress".to_string()];
        if prune {
            args.push("--prune".into());
        }
        if tags {
            args.push("--tags".into());
        }
        args.push(r.clone());
        let out = run_net(repo, r, args.clone(), progress.clone(), account).await?;
        if !out.ok() {
            failures.push((r.clone(), classify_git_failure(&args, &out.stderr)));
        }
    }
    if failures.len() == remotes.len() {
        let (r, e) = failures.remove(0);
        return Err(AppError { message: format!("Fetch de '{r}' falhou: {}", e.message), ..e });
    }
    let mut res = OperationResult::ok(
        if remotes.len() == 1 { format!("Fetch de '{}' concluído.", remotes[0]) } else { format!("Fetch de {} remotes concluído.", remotes.len()) },
        &["refs", "graph", "status"],
    );
    if !failures.is_empty() {
        res.message.push_str(&format!(" Falharam: {}.", failures.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>().join(", ")));
        res.details = Some(failures.iter().map(|(r, e)| format!("{r}: {}", e.details.clone().unwrap_or_default())).collect::<Vec<_>>().join("\n"));
    }
    Ok(res)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PullStrategy {
    /// Respect the repository/global git config (pull.rebase / pull.ff).
    Config,
    Merge,
    Rebase,
    FfOnly,
}

pub async fn pull(repo: &Repo, strategy: PullStrategy, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    let st = read::status(repo, false).await?;
    let Some(upstream) = st.head.upstream.clone() else {
        return Err(AppError::new(ErrorCode::NoUpstream, "A branch atual não tem upstream configurado; não há de onde fazer pull.")
            .recovery("Configure o tracking da branch (menu da branch > Definir upstream) ou faça Push com -u."));
    };
    if st.head.detached {
        return Err(AppError::invalid("HEAD destacado: faça checkout de uma branch antes do pull."));
    }
    let remote = upstream.split('/').next().unwrap_or("origin").to_string();
    let mut args = vec!["pull".to_string(), "--progress".to_string()];
    match strategy {
        PullStrategy::Config => {
            // Without any config modern git refuses divergent pulls; make the default explicit.
            let id = read::identity(repo).await;
            if id.pull_rebase.is_none() && id.pull_ff.is_none() {
                args.push("--no-rebase".into());
            }
        }
        PullStrategy::Merge => args.push("--no-rebase".into()),
        PullStrategy::Rebase => args.push("--rebase".into()),
        PullStrategy::FfOnly => args.push("--ff-only".into()),
    }
    let out = run_net(repo, &remote, args.clone(), progress, account).await?;
    if out.ok() {
        let text = out.text();
        let msg = if text.contains("Already up to date") { "Já está atualizado.".to_string() } else { format!("Pull de '{upstream}' concluído.") };
        return Ok(OperationResult::ok(msg, ALL).with_details(&out));
    }
    let combined = format!("{}\n{}", out.text(), out.stderr);
    if combined.contains("Not possible to fast-forward") || combined.contains("diverging branches") || combined.contains("divergent branches") {
        return Err(AppError::new(ErrorCode::NonFastForward, "A branch local e o upstream divergiram; fast-forward não é possível.")
            .details(combined)
            .recovery("Escolha Pull com merge ou com rebase."));
    }
    let gd = read::git_dir(repo).await.ok();
    if gd.as_deref().and_then(super::refs::detect_operation).is_some() {
        return Ok(OperationResult {
            ok: false,
            conflicts: true,
            message: "O pull parou com conflitos. Resolva os arquivos e escolha Continuar, ou Abortar.".into(),
            details: Some(crate::redact::redact(&combined)),
            changed: ALL.to_vec(),
        });
    }
    Err(classify_git_failure(&args, &combined).repo(&repo.display))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub remote: String,
    /// Local branch to push (defaults to current).
    pub branch: Option<String>,
    /// Remote branch name (defaults to the same name).
    pub remote_branch: Option<String>,
    #[serde(default)]
    pub set_upstream: bool,
    #[serde(default)]
    pub force_with_lease: bool,
    #[serde(default)]
    pub tags: bool,
    #[serde(default)]
    pub no_verify: bool,
}

pub async fn push(repo: &Repo, req: &PushRequest, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", &req.remote).await?;
    let head = read::head_ref(repo).await;
    let branch = match req.branch.clone().or(head.branch.clone()) {
        Some(b) => b,
        None => return Err(AppError::invalid("HEAD destacado: escolha uma branch para enviar.")),
    };
    check_ref_name(repo, "branch", &branch).await?;
    let remote_branch = req.remote_branch.clone().filter(|b| !b.trim().is_empty()).unwrap_or_else(|| branch.clone());
    check_ref_name(repo, "branch", &remote_branch).await?;
    let mut args = vec!["push".to_string(), "--progress".to_string(), "--porcelain".to_string()];
    if req.set_upstream {
        args.push("--set-upstream".into());
    }
    if req.force_with_lease {
        args.push("--force-with-lease".into());
    }
    if req.no_verify {
        args.push("--no-verify".into());
    }
    if req.tags {
        args.push("--follow-tags".into());
    }
    args.push(req.remote.clone());
    args.push(format!("refs/heads/{branch}:refs/heads/{remote_branch}"));
    let out = run_net(repo, &req.remote, args.clone(), progress, account).await?;
    if !out.ok() {
        let combined = format!("{}\n{}", out.text(), out.stderr);
        return Err(classify_git_failure(&args, &combined).repo(&repo.display));
    }
    let text = out.text();
    let msg = if text.contains("[up to date]") {
        "Nada para enviar: o remote já está atualizado.".to_string()
    } else if req.force_with_lease {
        format!("Force push (with lease) de '{branch}' para '{}/{remote_branch}' concluído.", req.remote)
    } else {
        format!("'{branch}' enviada para '{}/{remote_branch}'.", req.remote)
    };
    Ok(OperationResult::ok(msg, &["refs", "graph", "status"]).with_details(&out))
}

pub async fn delete_remote_branch(repo: &Repo, remote: &str, branch: &str, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", remote).await?;
    check_ref_name(repo, "branch", branch).await?;
    let args = vec!["push".to_string(), remote.to_string(), "--delete".to_string(), format!("refs/heads/{branch}")];
    let out = run_net(repo, remote, args.clone(), progress, account).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(format!("Branch remota '{remote}/{branch}' excluída."), &["refs", "graph"]))
}

pub async fn push_tag(repo: &Repo, remote: &str, tag: Option<&str>, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", remote).await?;
    let mut args = vec!["push".to_string(), remote.to_string()];
    match tag {
        Some(t) => {
            check_ref_name(repo, "tag", t).await?;
            args.push(format!("refs/tags/{t}:refs/tags/{t}"));
        }
        None => args.push("--tags".into()),
    }
    let out = run_net(repo, remote, args.clone(), progress, account).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(match tag { Some(t) => format!("Tag '{t}' enviada para '{remote}'."), None => format!("Tags enviadas para '{remote}'.") }, &["refs"]))
}

pub async fn delete_remote_tag(repo: &Repo, remote: &str, tag: &str, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", remote).await?;
    check_ref_name(repo, "tag", tag).await?;
    let args = vec!["push".to_string(), remote.to_string(), "--delete".to_string(), format!("refs/tags/{tag}")];
    let out = run_net(repo, remote, args.clone(), progress, account).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(format!("Tag '{tag}' excluída de '{remote}'."), &["refs"]))
}

pub async fn prune_remote(repo: &Repo, remote: &str, progress: Option<ProgressSink>, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", remote).await?;
    let args = vec!["remote".to_string(), "prune".to_string(), remote.to_string()];
    let out = run_net(repo, remote, args.clone(), progress, account).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(format!("Referências obsoletas de '{remote}' removidas."), &["refs", "graph"]).with_details(&out))
}

/// Checks whether git can authenticate against a remote without changing anything.
pub async fn test_remote(repo: &Repo, remote: &str, account: Option<&ProviderAccount>) -> AppResult<OperationResult> {
    check_ref_name(repo, "remote", remote).await?;
    let args = vec!["ls-remote".to_string(), "--heads".to_string(), remote.to_string()];
    let out = run_net(repo, remote, args.clone(), None, account).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok(format!("Conexão com '{remote}' OK (credenciais Git válidas)."), &[]))
}

pub async fn submodule_update(repo: &Repo, progress: Option<ProgressSink>) -> AppResult<OperationResult> {
    let args = vec!["submodule".to_string(), "update".to_string(), "--init".to_string(), "--recursive".to_string(), "--progress".to_string()];
    let out = repo.git.run(&repo.path, &args, RunOpts::network(progress)).await?;
    if !out.ok() {
        return Err(classify_git_failure(&args, &out.stderr));
    }
    Ok(OperationResult::ok("Submódulos atualizados.", &["status"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Provider;

    fn acc(p: Provider, base: Option<&str>) -> ProviderAccount {
        ProviderAccount {
            id: "x".into(),
            provider: p,
            base_url: base.map(str::to_string),
            username: "u".into(),
            display_name: None,
            avatar_url: None,
            email: None,
            label: None,
            created_at: 0,
            scopes: None,
            expires_at: None,
        }
    }

    #[test]
    fn token_only_used_for_matching_https_host() {
        let gh = acc(Provider::Github, None);
        assert!(account_for_url(Some(&gh), "https://github.com/a/b.git").is_some());
        assert!(account_for_url(Some(&gh), "https://evil.example/a/b.git").is_none());
        assert!(account_for_url(Some(&gh), "git@github.com:a/b.git").is_none());
        assert!(account_for_url(Some(&gh), "http://github.com/a/b.git").is_none());
        let gl = acc(Provider::Gitlab, Some("https://git.corp.io"));
        assert!(account_for_url(Some(&gl), "https://git.corp.io/g/p.git").is_some());
        assert!(account_for_url(Some(&gl), "https://gitlab.com/g/p.git").is_none());
    }
}
