//! The main window, created programmatically: macOS overlay title bar with
//! vibrancy, Windows frameless with Mica, Linux with a native shadow. The
//! window is transparent where a native material exists, and `Material`
//! tells the webview whether one is showing so it can stop painting over it.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{App, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Whether the native window material (vibrancy or Mica) was applied.
#[derive(Debug, Default)]
pub(crate) struct Material(AtomicBool);

impl Material {
    pub(crate) fn translucent(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub(crate) fn create_main(app: &App) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Z Engine")
        .inner_size(1100.0, 760.0)
        .min_inner_size(720.0, 520.0)
        .maximized(true);
    #[cfg(target_os = "macos")]
    {
        // tao's inset keeps the buttons near the bottom of a container of
        // height button_height + y, so y is about the vertical centre of
        // the 40px `.app-topbar`.
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(12.50, 22.50))
            .shadow(true)
            .transparent(true);
    }
    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false).transparent(true);
    }
    #[cfg(target_os = "linux")]
    {
        builder = builder.shadow(true);
    }
    let window = builder.build()?;
    let translucent = apply_material(&window);
    app.state::<Material>()
        .0
        .store(translucent, Ordering::Relaxed);
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_material(window: &WebviewWindow) -> bool {
    use window_vibrancy::{NSVisualEffectMaterial, apply_vibrancy};
    apply_vibrancy(
        window,
        NSVisualEffectMaterial::UnderWindowBackground,
        None,
        None,
    )
    .is_ok()
}

/// Mica needs Windows 11; on Windows 10 the webview keeps painting its own background.
#[cfg(target_os = "windows")]
fn apply_material(window: &WebviewWindow) -> bool {
    window_vibrancy::apply_mica(window, None).is_ok()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn apply_material(_window: &WebviewWindow) -> bool {
    false
}
