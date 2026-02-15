use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::icons;
use crate::state::AppState;

/// Menu item IDs we need to track for event handling.
pub struct MenuItems {
    pub toggle: MenuItem,
    pub quit: MenuItem,
}

/// Build the tray menu and items for the given state.
pub fn build_menu(state: &AppState) -> (Menu, MenuItems) {
    let menu = Menu::new();

    // Status line (disabled)
    let status = MenuItem::new(state.status_text(), false, None);

    // Toggle action (only shown for Paused/Listening)
    let toggle = MenuItem::new(state.toggle_label(), state.has_toggle(), None);

    let quit = MenuItem::new("Quit", true, None);

    if state.has_toggle() {
        menu.append_items(&[
            &status,
            &PredefinedMenuItem::separator(),
            &toggle,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .expect("failed to build menu");
    } else {
        menu.append_items(&[&status, &PredefinedMenuItem::separator(), &quit])
            .expect("failed to build menu");
    }

    (menu, MenuItems { toggle, quit })
}

/// Create the tray icon with the given state.
pub fn create_tray_icon(state: &AppState, menu: Menu) -> TrayIcon {
    let icon = select_icon_for_state(state);

    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Voice-Controllm")
        .with_icon(icon)
        .with_menu_on_left_click(true);

    #[cfg(target_os = "macos")]
    let builder = builder.with_icon_as_template(false);

    builder.build().expect("failed to create tray icon")
}

/// Select icon for the given state (public, for dynamic updates).
pub fn select_icon_for_state(state: &AppState) -> tray_icon::Icon {
    icons::load_icon(match state {
        AppState::Listening => include_bytes!("../icons/mic-listening@2x.png"),
        AppState::Paused => include_bytes!("../icons/mic-paused@2x.png"),
        AppState::Initializing { .. } => include_bytes!("../icons/mic-init@2x.png"),
        AppState::Disconnected | AppState::Error(_) => include_bytes!("../icons/mic-error@2x.png"),
    })
}
