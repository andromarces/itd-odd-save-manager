use tauri::{AppHandle, Manager, Runtime};

/// Helper to show and focus the main window.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>, _from_second_instance: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();

        // Foregrounding reliable on Windows and macOS, inconsistent on Linux
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let _ = window.set_focus();
        }

        #[cfg(target_os = "linux")]
        {
            use tauri_plugin_notification::NotificationExt;
            if _from_second_instance {
                let _ = app
                    .notification()
                    .builder()
                    .title("Already Running")
                    .body("ITD ODD Save Manager is already active.")
                    .show();
            }
        }
    }
}
