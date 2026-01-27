use crate::game_manager;
use crate::window::show_main_window;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{async_runtime, AppHandle, Manager, Runtime};

// Store tray icon to prevent it from being dropped
pub struct TrayState<R: Runtime>(pub tauri::tray::TrayIcon<R>);

/// Determines whether a tray icon event should show and focus the main window.
fn should_show_main_window_from_tray_event(event: &TrayIconEvent) -> bool {
    match event {
        TrayIconEvent::Click { button, .. } => matches!(button, tauri::tray::MouseButton::Left),
        TrayIconEvent::DoubleClick { button, .. } => {
            matches!(button, tauri::tray::MouseButton::Left)
        }
        _ => false,
    }
}

/// Resolves the tray tooltip text from the configured product name.
fn tray_tooltip_text(product_name: Option<&str>) -> String {
    product_name
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| "ITD ODD Save Manager".to_string())
}

/// Creates and configures the system tray icon and menu.
pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let status_i = MenuItem::with_id(app, "status", "Status: Monitoring", false, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let launch_i = MenuItem::with_id(app, "launch", "Launch Game", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&status_i, &open_i, &launch_i, &quit_i])?;

    let icon_bytes = include_bytes!("../icons/32x32.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes).expect("Failed to load icon");

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app: &AppHandle<R>, event: tauri::menu::MenuEvent| {
            match event.id().as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "open" => {
                    show_main_window(app, false);
                }
                "launch" => {
                    let app_handle = app.clone();
                    async_runtime::spawn(async move {
                        let _ = game_manager::launch_game(app_handle).await;
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event: TrayIconEvent| {
            if should_show_main_window_from_tray_event(&event) {
                show_main_window(tray.app_handle(), false);
            }
        })
        .icon(icon)
        .tooltip(tray_tooltip_text(app.config().product_name.as_deref()))
        .build(app)?;

    app.manage(TrayState(tray));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconId};

    /// Helper to create a click event.
    fn make_click_event(button: MouseButton) -> TrayIconEvent {
        TrayIconEvent::Click {
            button,
            button_state: MouseButtonState::Down,
            id: TrayIconId::new("test"),
            position: tauri::PhysicalPosition::default(),
            rect: tauri::Rect::default(),
        }
    }

    /// Helper to create a double click event.
    fn make_double_click_event(button: MouseButton) -> TrayIconEvent {
        TrayIconEvent::DoubleClick {
            button,
            id: TrayIconId::new("test"),
            position: tauri::PhysicalPosition::default(),
            rect: tauri::Rect::default(),
        }
    }

    /// Verifies that a left click triggers the main window.
    #[test]
    fn tray_left_click_shows_main_window() {
        let event = make_click_event(MouseButton::Left);
        assert!(should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a right click does not trigger the main window.
    #[test]
    fn tray_right_click_does_not_show_main_window() {
        let event = make_click_event(MouseButton::Right);
        assert!(!should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a left double click triggers the main window.
    #[test]
    fn tray_left_double_click_shows_main_window() {
        let event = make_double_click_event(MouseButton::Left);
        assert!(should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a right double click does not trigger the main window.
    #[test]
    fn tray_right_double_click_does_not_show_main_window() {
        let event = make_double_click_event(MouseButton::Right);
        assert!(!should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that the tray tooltip prefers the configured product name.
    #[test]
    fn tray_tooltip_uses_product_name_when_available() {
        let tooltip = tray_tooltip_text(Some("Configured Name"));
        assert_eq!(tooltip, "Configured Name");
    }

    /// Verifies that the tray tooltip falls back to the default name.
    #[test]
    fn tray_tooltip_falls_back_to_default_name() {
        let tooltip = tray_tooltip_text(None);
        assert_eq!(tooltip, "ITD ODD Save Manager");
    }
}
