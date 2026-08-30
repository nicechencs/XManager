//! XManager desktop entry (Tauri 2 + web frontend).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod commands;
mod dto;
mod state;

use std::sync::Mutex;

use xmanager_core::logging::{self, events, Outcome, Stream};

fn main() {
    let logging_ok = logging::init_best_effort();
    logging::info(Stream::App, events::APP_START)
        .outcome(if logging_ok {
            Outcome::Ok
        } else {
            Outcome::Error
        })
        .field("version", env!("CARGO_PKG_VERSION"))
        .field("file_logging", logging_ok)
        .emit();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::SharedWorkspace::new(Mutex::new(
            state::Workspace::new(),
        )))
        .invoke_handler(tauri::generate_handler![
            commands::initialize,
            commands::dispatch,
            commands::fetch_tweets,
            commands::refresh_whoami,
            commands::delete_previewed,
            commands::confirm_and_delete,
            commands::export_tweets,
            commands::export_candidates,
            commands::default_export_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
