use promptpack_marketplace::{eval, pack, registry, store, AppConfig, SharedState};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_config = AppConfig::default();
    let state: SharedState<AppConfig> = Arc::new(RwLock::new(app_config));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_clipboard::init())
        .plugin(tauri_plugin_notification::init())
        // global-shortcut uses different API in v2
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            pack::verify_pack,
            pack::create_pack,
            pack::extract_pack,
            pack::watch_packs_dir,
            eval::run_eval,
            eval::get_eval_results,
            registry::fetch_index,
            registry::submit_pack,
            registry::download_pack,
            store::get_config,
            store::set_config,
            store::save_eval_history,
            store::get_eval_history,
            store::get_installed_packs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}