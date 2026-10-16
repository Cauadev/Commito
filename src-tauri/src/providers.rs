//! GitHub / GitLab / Bitbucket Cloud REST integrations (account validation and repo listing).
//!
//! All HTTP happens in Rust; the WebView only sees profile metadata and repository lists.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde_json::Value;

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::settings::{Provider, ProviderAccount};

const UA: &str = concat!("ForgeGraph/", env!("CARGO_PKG_VERSION"));

pub fn http_client() -> Client {
    Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        // Never follow redirects to another host: tokens must stay with the configured server.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let same = attempt.previous().first().map(|u| u.host_str() == attempt.url().host_str() && u.scheme() == attempt.url().scheme()).unwrap_or(false);
            if same && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .expect("http client")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRepository {
    pub id: String,
    pub provider: Provider,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub private: bool,
    pub fork: bool,
    pub clone_https: Option<String>,
    pub clone_ssh: Option<String>,
    pub web_url: Option<String>,
    pub default_branch: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPage {
    pub items: Vec<RemoteRepository>,
    pub next_page: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub slug: String,
    pub name: String,
}

/// Validates and normalises a user-supplied base URL (https only, no credentials, no query).
pub fn normalize_base_url(provider: Provider, raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
    let url = url::Url::parse(raw).map_err(|_| AppError::invalid("URL base inválida."))?;
    if url.scheme() != "https" {
        return Err(AppError::invalid("A URL base precisa usar HTTPS."));
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() {
        return Err(AppError::invalid("A URL base não pode conter credenciais ou parâmetros."));
    }
    if provider == Provider::Bitbucket {
        return Err(AppError::invalid("Bitbucket Cloud não aceita URL base personalizada."));
    }
    let s = url.as_str().trim_end_matches('/').to_string();
    let default = match provider {
        Provider::Github => "https://api.github.com",
        Provider::Gitlab => "https://gitlab.com",
        Provider::Bitbucket => "https://api.bitbucket.org",
    };
    Ok(if s.eq_ignore_ascii_case(default) || (provider == Provider::Github && s.eq_ignore_ascii_case("https://github.com")) { None } else { Some(s) })
}

fn api_base(provider: Provider, base: Option<&str>) -> String {
    match (provider, base) {
        (Provider::Github, None) => "https://api.github.com".into(),
        // GitHub Enterprise Server exposes the REST API under /api/v3
        (Provider::Github, Some(b)) => {
            if b.ends_with("/api/v3") { b.to_string() } else { format!("{b}/api/v3") }
        }
        (Provider::Gitlab, None) => "https://gitlab.com/api/v4".into(),
        (Provider::Gitlab, Some(b)) => {
            if b.ends_with("/api/v4") { b.to_string() } else { format!("{b}/api/v4") }
        }
        (Provider::Bitbucket, _) => "https://api.bitbucket.org/2.0".into(),
    }
}

struct Auth<'a> {
    provider: Provider,
    token: &'a str,
    email: Option<&'a str>,
}

fn authorize(rb: RequestBuilder, auth: &Auth<'_>) -> RequestBuilder {
    match auth.provider {
        Provider::Github => rb
            .header(AUTHORIZATION, format!("Bearer {}", auth.token))
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28"),
        Provider::Gitlab => rb.header("PRIVATE-TOKEN", auth.token),
        Provider::Bitbucket => match auth.email {
            Some(email) => rb.basic_auth(email, Some(auth.token)).header(ACCEPT, "application/json"),
            None => rb.header(AUTHORIZATION, format!("Bearer {}", auth.token)).header(ACCEPT, "application/json"),
        },
    }
}

async fn check(resp: Response, provider: Provider) -> AppResult<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    let body_short: String = body.chars().take(600).collect();
    let name = provider_name(provider);
    Err(match status {
        StatusCode::UNAUTHORIZED => AppError::new(ErrorCode::AuthFailed, format!("{name}: token inválido, expirado ou revogado."))
            .details(body_short)
            .recovery(token_help(provider)),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS if is_rate_limited(&headers, status) => {
            let reset = header(&headers, "x-ratelimit-reset").or_else(|| header(&headers, "ratelimit-reset")).or_else(|| header(&headers, "retry-after"));
            AppError::new(ErrorCode::RateLimited, format!("{name}: limite de requisições da API atingido."))
                .details(body_short)
                .recovery(match reset {
                    Some(r) => format!("Tente novamente mais tarde (reset: {r})."),
                    None => "Aguarde alguns minutos e tente novamente.".into(),
                })
        }
        StatusCode::FORBIDDEN => AppError::new(ErrorCode::AuthFailed, format!("{name}: o token não tem permissão para esta operação."))
            .details(body_short)
            .recovery(token_help(provider)),
        StatusCode::NOT_FOUND => AppError::new(ErrorCode::NotFound, format!("{name}: recurso não encontrado (verifique URL/permissões)."))
            .details(body_short),
        _ => AppError::new(ErrorCode::Provider, format!("{name} respondeu HTTP {}.", status.as_u16())).details(body_short),
    })
}

fn header(h: &HeaderMap, k: &str) -> Option<String> {
    h.get(k).and_then(|v| v.to_str().ok()).map(str::to_string)
}

fn is_rate_limited(h: &HeaderMap, status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || h.get("x-ratelimit-remaining").map(|v| v == HeaderValue::from_static("0")).unwrap_or(false)
}

pub fn provider_name(p: Provider) -> &'static str {
    match p {
        Provider::Github => "GitHub",
        Provider::Gitlab => "GitLab",
        Provider::Bitbucket => "Bitbucket",
    }
}

pub fn token_help(p: Provider) -> &'static str {
    match p {
        Provider::Github => "Gere um Personal Access Token em github.com/settings/tokens. Classic: escopo 'repo' (privados) e 'read:org'. Fine-grained: acesso aos repositórios desejados com 'Metadata: read' e 'Contents: read/write'.",
        Provider::Gitlab => "Gere um Personal Access Token em Preferências > Access Tokens com 'read_api' (listar) e 'read_repository'/'write_repository' para Git.",
        Provider::Bitbucket => "Gere um API token com escopos em id.atlassian.com/manage-profile/security/api-tokens (Bitbucket): read:user, read:workspace, read:repository e write:repository. Use o e-mail da sua conta Atlassian.",
    }
}

async fn get_json(client: &Client, url: &str, auth: &Auth<'_>) -> AppResult<(Value, HeaderMap)> {
    let resp = authorize(client.get(url).header(USER_AGENT, UA), auth).send().await?;
    let resp = check(resp, auth.provider).await?;
    let headers = resp.headers().clone();
    let v = resp.json::<Value>().await?;
    Ok((v, headers))
}

fn s(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    match cur {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub async fn validate(client: &Client, provider: Provider, base_url: Option<&str>, email: Option<&str>, token: &str) -> AppResult<AccountProfile> {
    let api = api_base(provider, base_url);
    let auth = Auth { provider, token, email };
    match provider {
        Provider::Github => {
            let (v, h) = get_json(client, &format!("{api}/user"), &auth).await?;
            let scopes = header(&h, "x-oauth-scopes").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>());
            let mut warnings = vec![];
            if let Some(sc) = &scopes {
                if !sc.iter().any(|x| x == "repo") {
                    warnings.push("O token clássico não tem o escopo 'repo': repositórios privados não serão listados nem acessados via Git com este token.".into());
                }
            } else {
                warnings.push("Token fine-grained: apenas os repositórios concedidos ao token serão listados.".into());
            }
            Ok(AccountProfile {
                username: s(&v, &["login"]).unwrap_or_default(),
                display_name: s(&v, &["name"]),
                avatar_url: s(&v, &["avatar_url"]),
                scopes,
                expires_at: header(&h, "github-authentication-token-expiration"),
                warnings,
            })
        }
        Provider::Gitlab => {
            let (v, _) = get_json(client, &format!("{api}/user"), &auth).await?;
            let mut scopes = None;
            let mut expires = None;
            let mut warnings = vec![];
            if let Ok((t, _)) = get_json(client, &format!("{api}/personal_access_tokens/self"), &auth).await {
                scopes = t.get("scopes").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>());
                expires = s(&t, &["expires_at"]);
            }
            if let Some(sc) = &scopes {
                if !sc.iter().any(|x| x == "api" || x == "read_api") {
                    warnings.push("O token não tem 'read_api': a listagem de projetos pode falhar.".into());
                }
                if !sc.iter().any(|x| x == "api" || x == "write_repository") {
                    warnings.push("Sem 'write_repository': push via HTTPS com este token não funcionará.".into());
                }
            }
            Ok(AccountProfile {
                username: s(&v, &["username"]).unwrap_or_default(),
                display_name: s(&v, &["name"]),
                avatar_url: s(&v, &["avatar_url"]),
                scopes,
                expires_at: expires,
                warnings,
            })
        }
        Provider::Bitbucket => {
            if email.is_none_or(|e| e.trim().is_empty()) {
                return Err(AppError::invalid("Informe o e-mail da conta Atlassian para usar o API token."));
            }
            let (v, _) = get_json(client, &format!("{api}/user"), &auth).await?;
            let mut warnings = vec![];
            if get_json(client, &format!("{api}/user/workspaces?pagelen=1"), &auth).await.is_err() {
                warnings.push("Não foi possível listar workspaces: verifique o escopo read:workspace:bitbucket.".into());
            }
            Ok(AccountProfile {
                username: s(&v, &["username"]).or_else(|| s(&v, &["nickname"])).unwrap_or_default(),
                display_name: s(&v, &["display_name"]),
                avatar_url: s(&v, &["links", "avatar", "href"]),
                scopes: None,
                expires_at: None,
                warnings,
            })
        }
    }
}

pub async fn list_workspaces(client: &Client, account: &ProviderAccount, token: &str) -> AppResult<Vec<Workspace>> {
    match account.provider {
        Provider::Bitbucket => {
            let api = api_base(account.provider, None);
            let auth = Auth { provider: account.provider, token, email: account.email.as_deref() };
            let mut out = Vec::new();
            for page in 1..=20 {
                let (v, _) = get_json(client, &format!("{api}/user/workspaces?pagelen=100&page={page}"), &auth).await?;
                let values = v.get("values").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                for item in &values {
                    let ws = item.get("workspace").unwrap_or(item);
                    if let Some(slug) = s(ws, &["slug"]) {
                        out.push(Workspace { name: s(ws, &["name"]).unwrap_or_else(|| slug.clone()), slug });
                    }
                }
                if v.get("next").is_none() || values.is_empty() {
                    break;
                }
            }
            Ok(out)
        }
        _ => Ok(Vec::new()),
    }
}

pub async fn list_repos(client: &Client, account: &ProviderAccount, token: &str, page: u32, query: Option<&str>, workspace: Option<&str>) -> AppResult<RepoPage> {
    let api = api_base(account.provider, account.base_url.as_deref());
    let auth = Auth { provider: account.provider, token, email: account.email.as_deref() };
    let page = page.max(1);
    match account.provider {
        Provider::Github => {
            let url = format!("{api}/user/repos?per_page=100&page={page}&sort=updated&affiliation=owner,collaborator,organization_member");
            let (v, h) = get_json(client, &url, &auth).await?;
            let mut items: Vec<RemoteRepository> = v.as_array().cloned().unwrap_or_default().iter().map(|r| RemoteRepository {
                id: s(r, &["id"]).unwrap_or_default(),
                provider: Provider::Github,
                name: s(r, &["name"]).unwrap_or_default(),
                full_name: s(r, &["full_name"]).unwrap_or_default(),
                owner: s(r, &["owner", "login"]).unwrap_or_default(),
                description: s(r, &["description"]),
                private: r.get("private").and_then(|x| x.as_bool()).unwrap_or(false),
                fork: r.get("fork").and_then(|x| x.as_bool()).unwrap_or(false),
                clone_https: s(r, &["clone_url"]),
                clone_ssh: s(r, &["ssh_url"]),
                web_url: s(r, &["html_url"]),
                default_branch: s(r, &["default_branch"]),
                updated_at: s(r, &["pushed_at"]).or_else(|| s(r, &["updated_at"])),
            }).collect();
            if let Some(q) = query.map(str::to_lowercase).filter(|q| !q.is_empty()) {
                items.retain(|r| r.full_name.to_lowercase().contains(&q) || r.description.as_deref().unwrap_or("").to_lowercase().contains(&q));
            }
            let has_next = header(&h, "link").map(|l| l.contains("rel=\"next\"")).unwrap_or(false);
            Ok(RepoPage { items, next_page: has_next.then_some(page + 1) })
        }
        Provider::Gitlab => {
            let mut url = format!("{api}/projects?membership=true&simple=true&per_page=100&page={page}&order_by=last_activity_at");
            if let Some(q) = query.filter(|q| !q.is_empty()) {
                url.push_str(&format!("&search={}", urlencoding::encode(q)));
            }
            let (v, h) = get_json(client, &url, &auth).await?;
            let items = v.as_array().cloned().unwrap_or_default().iter().map(|r| RemoteRepository {
                id: s(r, &["id"]).unwrap_or_default(),
                provider: Provider::Gitlab,
                name: s(r, &["name"]).unwrap_or_default(),
                full_name: s(r, &["path_with_namespace"]).unwrap_or_default(),
                owner: s(r, &["namespace", "full_path"]).unwrap_or_default(),
                description: s(r, &["description"]),
                private: s(r, &["visibility"]).map(|v| v != "public").unwrap_or(true),
                fork: r.get("forked_from_project").is_some_and(|x| !x.is_null()),
                clone_https: s(r, &["http_url_to_repo"]),
                clone_ssh: s(r, &["ssh_url_to_repo"]),
                web_url: s(r, &["web_url"]),
                default_branch: s(r, &["default_branch"]),
                updated_at: s(r, &["last_activity_at"]),
            }).collect();
            let next = header(&h, "x-next-page").and_then(|n| n.parse::<u32>().ok());
            Ok(RepoPage { items, next_page: next })
        }
        Provider::Bitbucket => {
            let Some(ws) = workspace.filter(|w| !w.is_empty()) else {
                return Err(AppError::invalid("Selecione um workspace do Bitbucket."));
            };
            let mut url = format!("{api}/repositories/{}?pagelen=100&page={page}&sort=-updated_on", urlencoding::encode(ws));
            if let Some(q) = query.filter(|q| !q.is_empty()) {
                let escaped = q.replace('\\', "\\\\").replace('"', "\\\"");
                url.push_str(&format!("&q={}", urlencoding::encode(&format!("name ~ \"{escaped}\""))));
            }
            let (v, _) = get_json(client, &url, &auth).await?;
            let values = v.get("values").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let items = values.iter().map(|r| {
                let clone = r.get("links").and_then(|l| l.get("clone")).and_then(|c| c.as_array()).cloned().unwrap_or_default();
                let find = |n: &str| clone.iter().find(|c| s(c, &["name"]).as_deref() == Some(n)).and_then(|c| s(c, &["href"]));
                // Drop the embedded username from the https clone URL; auth comes from GCM / askpass.
                let https = find("https").map(|u| match url::Url::parse(&u) {
                    Ok(mut parsed) => { let _ = parsed.set_username(""); parsed.to_string() }
                    Err(_) => u,
                });
                RemoteRepository {
                    id: s(r, &["uuid"]).unwrap_or_default(),
                    provider: Provider::Bitbucket,
                    name: s(r, &["name"]).unwrap_or_default(),
                    full_name: s(r, &["full_name"]).unwrap_or_default(),
                    owner: s(r, &["workspace", "slug"]).unwrap_or_default(),
                    description: s(r, &["description"]).filter(|d| !d.is_empty()),
                    private: r.get("is_private").and_then(|x| x.as_bool()).unwrap_or(true),
                    fork: r.get("parent").is_some_and(|x| !x.is_null()),
                    clone_https: https,
                    clone_ssh: find("ssh"),
                    web_url: s(r, &["links", "html", "href"]),
                    default_branch: s(r, &["mainbranch", "name"]),
                    updated_at: s(r, &["updated_on"]),
                }
            }).collect();
            let next = v.get("next").and_then(|n| n.as_str()).map(|_| page + 1);
            Ok(RepoPage { items, next_page: next })
        }
    }
}

/// Host used for Git HTTPS by this account (to decide whether its token applies to a remote).
pub fn git_host(account: &ProviderAccount) -> String {
    match (account.provider, account.base_url.as_deref()) {
        (Provider::Github, None) => "github.com".into(),
        (Provider::Gitlab, None) => "gitlab.com".into(),
        (Provider::Bitbucket, _) => "bitbucket.org".into(),
        (_, Some(b)) => url::Url::parse(b).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_validation() {
        assert!(normalize_base_url(Provider::Gitlab, Some("http://gitlab.local")).is_err());
        assert!(normalize_base_url(Provider::Gitlab, Some("https://u:p@gitlab.local")).is_err());
        assert_eq!(normalize_base_url(Provider::Gitlab, Some("https://gitlab.com/")).unwrap(), None);
        assert_eq!(normalize_base_url(Provider::Gitlab, Some("https://git.corp.io/")).unwrap().as_deref(), Some("https://git.corp.io"));
        assert_eq!(api_base(Provider::Gitlab, Some("https://git.corp.io")), "https://git.corp.io/api/v4");
        assert_eq!(api_base(Provider::Github, Some("https://ghe.corp.io")), "https://ghe.corp.io/api/v3");
        assert!(normalize_base_url(Provider::Bitbucket, Some("https://x.y")).is_err());
    }
}
