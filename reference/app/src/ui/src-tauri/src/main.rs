//! a2dpwb.exe: the A2DPWB window process. It shows the state of
//! a2dpwb-core.exe and sends it the user's requests; the core keeps
//! streaming when this process ends.
//!
//! Arguments: `--flyout` opens the tray flyout instead of the main window
//! (the core starts the UI this way when the tray icon is clicked).

#![forbid(unsafe_code)]
// No console window in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod connection;
mod update;
mod windows;

use tauri::Manager;

fn main() {
    let flyout = std::env::args().any(|a| a == "--flyout");
    let ipc = connection::Ipc::new();
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch ends before anything
        // else starts
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| windows::on_second_instance(app, &args)))
        // Its commands are not open to the webview (capabilities): updates
        // go through update.rs only
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ipc.clone())
        .invoke_handler(tauri::generate_handler![
            commands::ipc_request,
            commands::ipc_status,
            commands::ui_info,
            commands::show_main,
            commands::show_flyout,
            commands::set_theme,
            commands::open_url,
            commands::open_folder,
            commands::reveal_path,
            commands::update_status,
            commands::update_install,
        ])
        .on_window_event(windows::on_window_event)
        .setup(move |app| {
            let handle = app.handle().clone();
            app.manage(update::Updates::new(&handle));
            connection::start(handle.clone(), ipc);
            if flyout {
                windows::show_flyout(&handle)?;
            } else {
                windows::show_main(&handle, None)?;
            }
            windows::start_idle_exit(handle);
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(e) => {
            eprintln!("a2dpwb: cannot start the window: {e}");
            std::process::exit(1);
        }
    };
    app.run(|_, event| {
        // Windows are hidden, not closed (see windows.rs), so this comes
        // only from app.exit() or the last window being destroyed by the
        // system; only the explicit exit ends the process
        if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
            api.prevent_exit();
        }
    });
}
