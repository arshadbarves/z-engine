use tauri::Emitter;
use z_engine_core::agent::EventRx;

pub(crate) fn forward_events(mut rx: EventRx, window: tauri::WebviewWindow, session_id: String) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let mut payload = serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "sessionId".into(),
                    serde_json::Value::String(session_id.clone()),
                );
            }
            if window.emit("appEvent", payload).is_err() {
                break;
            }
        }
    });
}
