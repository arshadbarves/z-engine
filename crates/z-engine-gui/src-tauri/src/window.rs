//! The main window, created programmatically: macOS overlay title bar with
//! vibrancy, Windows frameless with Mica, Linux with a native shadow.

use tauri::{App, WebviewUrl, WebviewWindowBuilder};

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
            .shadow(true);
    }
    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false);
    }
    #[cfg(target_os = "linux")]
    {
        builder = builder.shadow(true);
    }
    let window = builder.build()?;
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{NSVisualEffectMaterial, apply_vibrancy};
        let _ = apply_vibrancy(
            &window,
            NSVisualEffectMaterial::UnderWindowBackground,
            None,
            None,
        );
    }
    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::apply_mica(&window, None);
    }
    #[cfg(target_os = "linux")]
    let _ = window;
    Ok(())
}
