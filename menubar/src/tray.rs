use tracing::warn;
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};
use vcm_common::bundle::VcmctlInstallState;

use crate::icons;
use crate::state::{AppState, LanguageInfo};

/// Menu item IDs we need to track for event handling.
pub struct MenuItems {
    pub toggle: MenuItem,
    /// Language check menu items: each entry is (CheckMenuItem, language code).
    pub language_items: Vec<(CheckMenuItem, String)>,
    /// `None` when vcmctl is already installed (item is omitted from the menu).
    pub install_cli: Option<MenuItem>,
    pub quit: MenuItem,
}

/// Build the tray menu and items for the given state and language info.
pub fn build_menu(state: &AppState, language: &LanguageInfo) -> (Menu, MenuItems) {
    let menu = Menu::new();

    // Status line (disabled) — includes active language for operational states
    let status = MenuItem::new(
        state.status_text_with_language(&language.active),
        false,
        None,
    );

    // Toggle action (only shown for Paused/Listening)
    let toggle = MenuItem::new(state.toggle_label(), state.has_toggle(), None);

    let install_state = std::env::current_exe()
        .map(|exe| vcm_common::bundle::vcmctl_install_state(&exe))
        .unwrap_or(VcmctlInstallState::NotInstalled);
    if install_state == VcmctlInstallState::ConflictingFile {
        warn!(
            path = "~/.local/bin/vcmctl",
            "vcmctl exists at the install path but is not a symlink; \
             the user may have placed their own binary there"
        );
    }
    let install_cli = (install_state != VcmctlInstallState::Installed)
        .then(|| MenuItem::new("Install vcmctl in PATH", true, None));
    let quit = MenuItem::new("Quit", true, None);

    // Build language items if there are available languages


    // Assemble the menu
    menu.append_items(&[&status, &PredefinedMenuItem::separator()])
        .expect("failed to build menu");

    if state.has_toggle() {
        menu.append_items(&[&toggle, &PredefinedMenuItem::separator()])
            .expect("failed to build menu");
    }

    if !language_items.is_empty() {
        let label = MenuItem::new("Language", false, None);
        menu.append_items(&[&label]).expect("failed to build menu");
        for (item, _code) in &language_items {
            menu.append_items(&[item]).expect("failed to build menu");
        }
        menu.append_items(&[&PredefinedMenuItem::separator()])
            .expect("failed to build menu");
    }

    if let Some(install_cli) = &install_cli {
        menu.append_items(&[install_cli, &PredefinedMenuItem::separator()])
            .expect("failed to build menu");
    }
    menu.append_items(&[&quit]).expect("failed to build menu");

    (
        menu,
        MenuItems {
            toggle,
            language_items,
            install_cli,
            quit,
        },
    )
}

/// Build check menu items for each available language plus "auto".
fn build_language_items(language: &LanguageInfo) -> Vec<(CheckMenuItem, String)> {
    if language.available.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();

    for lang in &language.available {
        let checked = language.active.matches_code(lang);
        let item = CheckMenuItem::new(lang, true, checked, None);
        items.push((item, lang.clone()));
    }

    // Always include "auto" if not already present
    let has_auto = language
        .available
        .iter()
        .any(|l| l.eq_ignore_ascii_case("auto"));
    if !has_auto {
        let checked = language.active.matches_code("auto");
        let item = CheckMenuItem::new("auto", true, checked, None);
        items.push((item, "auto".to_string()));
    }

    items
}

/// Create the tray icon with the given state.
pub fn create_tray_icon(state: &AppState, menu: Menu) -> TrayIcon {
    let icon = select_icon_for_state(state);

    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("VCM")
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
