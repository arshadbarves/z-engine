use harness_core::agent::EventRx;
use tauri::Emitter;

pub(crate) fn forward_events(mut rx: EventRx, window: tauri::WebviewWindow) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let payload = serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null);
            if window.emit("appEvent", payload).is_err() {
                break;
            }
        }
    });
}
