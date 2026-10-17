//! The real executable acts as GIT_ASKPASS: it must print the vault token for "Password" prompts
//! and the provider username for "Username" prompts, without starting the UI.

use std::process::Command;

#[test]
fn askpass_bridge_reads_token_from_os_vault() {
    let id = format!("test-askpass-{}", std::process::id());
    commito_lib::credentials::store_token(&id, "s3cret-token").expect("store token in vault");

    let exe = env!("CARGO_BIN_EXE_commito");
    let run = |prompt: &str| {
        let out = Command::new(exe)
            .arg(prompt)
            .env(commito_lib::credentials::ASKPASS_ACCOUNT_ENV, &id)
            .env(commito_lib::credentials::ASKPASS_USER_ENV, "x-access-token")
            .output()
            .expect("run askpass");
        (out.status.code(), String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let user = run("Username for 'https://github.com': ");
    let pass = run("Password for 'https://x-access-token@github.com': ");
    let other = run("Something else?");

    commito_lib::credentials::delete_token(&id).expect("delete token");
    assert_eq!(user, (Some(0), "x-access-token".to_string()));
    assert_eq!(pass, (Some(0), "s3cret-token".to_string()));
    assert_eq!(other.0, Some(1));
    assert!(commito_lib::credentials::read_token(&id).is_err(), "token must be gone after delete");
}
