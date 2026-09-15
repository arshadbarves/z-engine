fn sse_event(json: &str) -> String {
    format!("data: {json}\n\n")
}

pub(crate) fn text_delta(t: &str) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"content":"{t}"}}}}]}}"#
    ))
}

pub(crate) fn finish_json(reason: &str, prompt: u64, completion: u64) -> String {
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{}},"finish_reason":"{reason}"}}],"usage":{{"prompt_tokens":{prompt},"completion_tokens":{completion}}}}}"#
    ))
}

/// One tool-call delta fragment. `args` is raw JSON text (may be a partial).
pub(crate) fn tool_call_delta(
    index: usize,
    id: Option<&str>,
    name: Option<&str>,
    args: &str,
) -> String {
    let escaped = args.replace('\\', "\\\\").replace('"', "\\\"");
    let id_part = id.map(|i| format!(r#""id":"{i}","#)).unwrap_or_default();
    let fn_name = name
        .map(|n| format!(r#""name":"{n}","#))
        .unwrap_or_default();
    sse_event(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":{index},{id_part}"type":"function","function":{{{fn_name}"arguments":"{escaped}"}}}}]}}}}]}}"#
    ))
}

pub(crate) fn done() -> String {
    "data: [DONE]\n\n".to_string()
}
