mod auth;
mod commands;
#[cfg(target_os = "macos")]
mod macos_chrome;

use tauri::menu::{AboutMetadataBuilder, Menu, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn start_new_chat(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("vibn://new-chat", ());
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Hide native traffic lights — we render custom ones in the
            // titlebar. macOS keeps drawing them even with decorations:false
            // until we explicitly call setHidden on each.
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                let w0 = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    let w_for_main = w0.clone();
                    let _ = w0.run_on_main_thread(move || {
                        macos_chrome::hide_native_traffic_lights(&w_for_main);
                    });
                });
                let w = window.clone();
                window.on_window_event(move |event| {
                    if matches!(
                        event,
                        tauri::WindowEvent::Resized(_)
                            | tauri::WindowEvent::Moved(_)
                            | tauri::WindowEvent::Focused(_)
                    ) {
                        macos_chrome::hide_native_traffic_lights(&w);
                    }
                });
            }
            let handle = app.handle();
            let about = AboutMetadataBuilder::new()
                .name(Some("Vibn"))
                .version(Some(env!("CARGO_PKG_VERSION")))
                .build();
            let app_menu = SubmenuBuilder::new(handle, "Vibn")
                .about(Some(about))
                .separator()
                .text("app_show", "Show Vibn")
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;
            let file_menu = SubmenuBuilder::new(handle, "File")
                .text("app_new_chat", "New Chat")
                .separator()
                .close_window()
                .build()?;
            let edit_menu = SubmenuBuilder::new(handle, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let window_menu = SubmenuBuilder::new(handle, "Window")
                .minimize()
                .fullscreen()
                .separator()
                .bring_all_to_front()
                .build()?;
            let menubar = MenuBuilder::new(handle)
                .item(&app_menu)
                .item(&file_menu)
                .item(&edit_menu)
                .item(&window_menu)
                .build()?;
            app.set_menu(menubar)?;
            app.on_menu_event(|app, event| match event.id.as_ref() {
                "app_show" => show_main_window(app),
                "app_new_chat" => start_new_chat(app),
                _ => {}
            });

            let show_item = MenuItem::with_id(handle, "show", "Show Vibn", true, None::<&str>)?;
            let new_chat = MenuItem::with_id(handle, "new_chat", "New Chat", true, None::<&str>)?;
            let sep = tauri::menu::PredefinedMenuItem::separator(handle)?;
            let quit = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&show_item, &new_chat, &sep, &quit])?;

            let icon = app.default_window_icon().cloned();
            let mut builder = TrayIconBuilder::with_id("vibn-tray")
                .icon_as_template(true)
                .tooltip("Vibn")
                .menu(&menu)
                .on_menu_event(|app: &tauri::AppHandle, event| match event.id.as_ref() {
                    "show" => {
                        show_main_window(app);
                    }
                    "new_chat" => {
                        start_new_chat(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(event, TrayIconEvent::Click { .. }) {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = icon {
                builder = builder.icon(icon);
            }
            let _tray = builder.build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::active_model,
            commands::set_active_model,
            commands::list_transcripts,
            commands::load_transcript,
            commands::send_message,
            commands::get_config,
            commands::set_config_field,
            commands::read_image_as_data_url,
            commands::copy_image_to_clipboard,
            commands::save_image_as,
            commands::new_session_id,
            commands::list_slash_commands,
            commands::install_comfyui_cmd,
            commands::start_comfyui_cmd,
            commands::stop_comfyui_cmd,
            commands::download_image_model_cmd,
            auth::get_user_profile,
            auth::save_user_profile,
            auth::save_credential,
            auth::get_credential,
            auth::delete_credential,
            auth::sign_in,
            auth::sign_up,
            auth::sign_out,
            commands::list_mcp_servers,
            commands::add_mcp_server,
            commands::remove_mcp_server,
            commands::list_project_memory,
            commands::save_observation,
            commands::forget_project_memory,
            commands::token_usage,
            commands::run_slash_text,
            commands::list_model_registry,
            commands::pull_ollama_model,
            commands::delete_transcript,
            commands::get_active_project,
            commands::set_active_project,
            commands::clear_active_project,
            commands::forget_recent_project,
            commands::scan_project,
            commands::list_project_files,
            commands::read_project_file,
            commands::write_project_file,
            commands::drain_editor_events,
            commands::check_desktop_permissions,
            commands::open_system_settings_pane,
        ])
        .run(tauri::generate_context!())
        .expect("error while running vibn-desktop");
}
