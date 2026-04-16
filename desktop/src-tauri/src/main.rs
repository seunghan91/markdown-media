#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod export;
mod history;
mod markdown;
mod models;

use commands::{batch, convert, export as export_commands, rhwp_edit, viewer};
use history::HistoryStore;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        // --- Plugins ---
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        // single-instance disabled for dev — re-enable for production
        // .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        //     if let Some(window) = app.get_webview_window("main") {
        //         let _ = window.show();
        //         let _ = window.set_focus();
        //     }
        // }))
        // --- State ---
        .manage(HistoryStore::new().expect("failed to initialize history store"))
        // --- IPC Commands ---
        .invoke_handler(tauri::generate_handler![
            convert::convert_file,
            convert::convert_text,
            export_commands::export_to_docx,
            export_commands::export_to_hwpx,
            export_commands::export_to_pdf,
            batch::batch_convert,
            viewer::open_file,
            viewer::get_markdown_source,
            rhwp_edit::rhwp_list_paragraphs,
            rhwp_edit::rhwp_save_with_edits,
            get_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running mdm desktop");
}

#[tauri::command]
fn get_history(
    limit: usize,
    history: tauri::State<'_, HistoryStore>,
) -> Result<Vec<models::HistoryEntry>, String> {
    history.recent(limit)
}
