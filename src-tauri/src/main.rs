use promptpack_marketplace::{eval, pack, registry, store};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            pack::verify_pack,
            pack::create_pack,
            pack::extract_pack,
            eval::run_eval,
            eval::get_eval_results,
            registry::fetch_index,
            registry::submit_pack,
            store::get_config,
            store::set_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
