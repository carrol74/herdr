#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{configure_window, primary_mouse_button_down, set_prompt_active};

#[cfg(not(target_os = "macos"))]
mod other;
#[cfg(not(target_os = "macos"))]
pub use other::{configure_window, primary_mouse_button_down, set_prompt_active};
