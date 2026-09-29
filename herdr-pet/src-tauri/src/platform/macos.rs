use objc2_app_kit::{
    NSEvent, NSFloatingWindowLevel, NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior,
};
use tauri::WebviewWindow;

pub fn configure_window(window: &WebviewWindow) -> tauri::Result<()> {
    let pointer = window.ns_window()?;
    let window = unsafe { &*pointer.cast::<NSWindow>() };
    let behavior = window.collectionBehavior()
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    window.setCanHide(false);
    window.setHidesOnDeactivate(false);
    window.setCollectionBehavior(behavior);
    window.setLevel(NSScreenSaverWindowLevel);
    window.setHasShadow(false);
    Ok(())
}

pub fn primary_mouse_button_down() -> bool {
    NSEvent::pressedMouseButtons() & 1 != 0
}

pub fn set_prompt_active(window: &WebviewWindow, active: bool) -> tauri::Result<()> {
    let pointer = window.ns_window()?;
    let window = unsafe { &*pointer.cast::<NSWindow>() };
    window.setLevel(if active {
        NSFloatingWindowLevel
    } else {
        NSScreenSaverWindowLevel
    });
    Ok(())
}
