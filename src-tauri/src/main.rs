// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // When git launches this executable as GIT_ASKPASS, answer the prompt and exit
    // without starting the UI.
    if let Some(code) = commito_lib::credentials::maybe_run_askpass() {
        std::process::exit(code);
    }
    commito_lib::run()
}
