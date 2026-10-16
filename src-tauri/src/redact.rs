//! Secret redaction for anything that may reach logs, error details or the UI.

const TOKEN_PREFIXES: &[&str] = &[
    "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "gloas-", "glrt-", "ATATT", "ATCTT",
];

/// Masks credentials embedded in URLs (`scheme://user:secret@host`) and well-known
/// token formats (GitHub, GitLab, Atlassian).
pub fn redact(input: &str) -> String {
    let mut out = mask_url_userinfo(input);
    for prefix in TOKEN_PREFIXES {
        out = mask_prefixed_tokens(&out, prefix);
    }
    out
}

fn mask_url_userinfo(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find("://") {
        let (head, tail) = rest.split_at(pos + 3);
        out.push_str(head);
        // userinfo ends at '@' but only if it appears before the next '/', whitespace or quote.
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        if let Some(at) = authority.rfind('@') {
            let userinfo = &authority[..at];
            if let Some(colon) = userinfo.find(':') {
                out.push_str(&userinfo[..colon]);
                out.push_str(":***");
            } else {
                out.push_str(userinfo);
            }
            out.push_str(&authority[at..]);
            rest = &tail[end..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn mask_prefixed_tokens(input: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(prefix) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let len = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '='))
            .unwrap_or(after.len());
        if len > prefix.len() + 6 {
            out.push_str(prefix);
            out.push_str("***");
        } else {
            out.push_str(&after[..len]);
        }
        rest = &after[len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn masks_url_credentials() {
        assert_eq!(
            redact("fatal: unable to access 'https://bob:s3cr3t@github.com/a/b.git/'"),
            "fatal: unable to access 'https://bob:***@github.com/a/b.git/'"
        );
        assert_eq!(redact("https://github.com/a/b"), "https://github.com/a/b");
        assert_eq!(redact("ssh://git@host:22/x"), "ssh://git@host:22/x");
    }

    #[test]
    fn masks_known_tokens() {
        assert_eq!(redact("token ghp_abcdefghijklmnopqrstuvwxyz123456 end"), "token ghp_*** end");
        assert_eq!(redact("glpat-AbCdEfGhIjKlMnOp"), "glpat-***");
        assert_eq!(redact("ghp_ short"), "ghp_ short");
    }
}
