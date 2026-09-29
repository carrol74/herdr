use tauri::WebviewWindow;

pub fn configure_window(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

pub fn primary_mouse_button_down() -> bool {
    false
}

pub fn set_prompt_active(_window: &WebviewWindow, _active: bool) -> tauri::Result<()> {
    Ok(())
}
