//! The popover is a non-activating NSPanel, not a window.
//!
//! Two things a plain window cannot do, and the popover needs both. It has to
//! appear on whatever Space the user is looking at — a window belongs to the
//! Space it was born on, so on any other desktop the click did nothing at all.
//! And it has to stay up while Nod is not the active app: Nod is an accessory
//! with no Dock icon, so `set_focus` on a plain window never really takes, macOS
//! sends "you lost focus" a moment later, and the popover blinks and is gone.
//!
//! A panel with the NonactivatingPanel mask sidesteps both — the same mechanism
//! Spotlight and Raycast use. Nothing here types, so Nod never steals activation
//! from the app underneath; clicks land on a non-key window fine.

#[cfg(target_os = "macos")]
use tauri::Manager as _;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(NodPanel {
        config: {
            can_become_key_window: false,  // nothing to type into; keep the app below active
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

#[cfg(target_os = "macos")]
pub fn setup_panel(window: &tauri::WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::{CollectionBehavior, StyleMask, WebviewWindowExt};

    let panel = window.to_panel::<NodPanel>().map_err(|e| e.to_string())?;
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );
    // Nod is never the active app, so "deactivated" is not a reason to close.
    panel.set_hides_on_deactivate(false);
    crate::debug_log::log("panel: popover converted to non-activating NSPanel");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn setup_panel(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn show_popover(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    match app.get_webview_panel(crate::POPOVER) {
        Ok(p) => p.show(),
        Err(e) => crate::debug_log::log(&format!("popover: panel missing ({:?})", e)),
    }
}

#[cfg(target_os = "macos")]
pub fn hide_popover(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(p) = app.get_webview_panel(crate::POPOVER) {
        p.hide();
    }
}

#[cfg(target_os = "macos")]
pub fn popover_visible(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;
    app.get_webview_panel(crate::POPOVER)
        .map(|p| p.is_visible())
        .unwrap_or(false)
}

/// Puts the popover away when the user clicks anywhere else.
///
/// A non-activating panel never becomes the active app, so there is no "lost
/// focus" callback to hang this on. A global NSEvent monitor reports mouse-downs
/// that landed in *other* applications and never fires for our own — so the
/// cross inside the popover cannot dismiss the popover out from under itself,
/// and neither can the menu-bar icon. Mouse monitors need no Accessibility
/// grant; only keyboard ones do.
#[cfg(target_os = "macos")]
pub fn dismiss_on_outside_click(app: tauri::AppHandle) {
    use block::ConcreteBlock;
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};

    const LEFT_MOUSE_DOWN: u64 = 1 << 1;
    const RIGHT_MOUSE_DOWN: u64 = 1 << 3;
    const OTHER_MOUSE_DOWN: u64 = 1 << 25;

    let handler = ConcreteBlock::new(move |_event: id| {
        if popover_visible(&app) {
            hide_popover(&app);
        }
    });
    // The monitor outlives this call and keeps calling the block, so the block
    // has to outlive it too — copied to the heap and deliberately never freed.
    let handler = handler.copy();
    unsafe {
        let mask = LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN;
        let _: id = msg_send![class!(NSEvent),
            addGlobalMonitorForEventsMatchingMask: mask
            handler: &*handler];
    }
    std::mem::forget(handler);
    crate::debug_log::log("panel: watching for clicks outside the popover");
}

// Windows keeps the ordinary window: clicking another one takes focus away from
// it, which the focus handler in lib.rs already turns into a hide.
#[cfg(not(target_os = "macos"))]
pub fn dismiss_on_outside_click(_app: tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn show_popover(app: &tauri::AppHandle) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window(crate::POPOVER) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_popover(app: &tauri::AppHandle) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window(crate::POPOVER) {
        let _ = w.hide();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn popover_visible(app: &tauri::AppHandle) -> bool {
    use tauri::Manager as _;
    app.get_webview_window(crate::POPOVER)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}
