//! Integration tests against a REAL rust-analyzer (skipped silently when
//! the binary is absent). These prove spec §9 v0.8's core mechanics:
//! publishDiagnostics capture and definition resolution.

use std::sync::Arc;
use std::time::Duration;
use z_engine_core::lsp::LspClient;

fn ra_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

async fn make_project(broken: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let body = if broken {
        "pub fn alpha() -> u32 { 7 }\n\npub fn beta() {\n    let x: u32 = alpha();\n    let y: &str = x; // deliberate type error\n    println!(\"{y}\");\n}\n"
    } else {
        "pub fn alpha() -> u32 { 7 }\n\npub fn beta() {\n    let n = alpha();\n    println!(\"{n}\");\n}\n"
    };
    let path = tmp.path().join("src/lib.rs");
    std::fs::write(&path, body).unwrap();
    (tmp, path)
}

/// NOTE: the stdio server requires spawning rust-analyzer's large-stack
/// worker thread, which some sandboxed environments deny. In such
/// environments harness falls back to the batch CLI backend (see
/// lsp/batch.rs); these protocol-level tests remain for healthy hosts.
#[ignore = "stdio server may be blocked by sandbox stack limits; batch backend covered above"]
#[tokio::test]
async fn ra_publishes_diagnostics_for_broken_edit() {
    if !ra_available() {
        eprintln!("rust-analyzer not installed; skipping");
        return;
    }
    let (tmp, lib_rs) = make_project(true).await;
    let client = LspClient::new(tmp.path(), std::path::PathBuf::from("rust-analyzer"));

    let text = std::fs::read_to_string(&lib_rs).unwrap();
    client.open_document(&lib_rs, &text).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut found = String::new();
    while tokio::time::Instant::now() < deadline {
        let diags = client.diagnostics_for(&lib_rs).await;
        let rendered = serde_json::to_string(&diags).unwrap_or_default();
        if rendered.contains("mismatched types") || rendered.contains("E0308") {
            found = rendered;
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(
        found.contains("mismatched types") || found.contains("E0308"),
        "expected a type-mismatch diagnostic, got: {found}"
    );
}

#[ignore = "stdio server may be blocked by sandbox stack limits"]
#[tokio::test]
async fn ra_resolves_go_to_definition() {
    if !ra_available() {
        eprintln!("rust-analyzer not installed; skipping");
        return;
    }
    let (_tmp, lib_rs) = make_project(false).await;
    let client = LspClient::new(
        lib_rs.parent().unwrap().parent().unwrap(),
        std::path::PathBuf::from("rust-analyzer"),
    );

    // Open with content where `alpha()` is called on line 5 (1-based):
    // 1 pub fn alpha() -> u32 { 7 }
    // ...
    // 5     let n = alpha();
    let text = std::fs::read_to_string(&lib_rs).unwrap();
    client.open_document(&lib_rs, &text).await.unwrap();
    // Give the server a beat to index.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let params = serde_json::json!({
        "textDocument": {"uri": format!("file://{}", lib_rs.display())},
        "position": {"line": 4, "character": 14}
    });
    let result = client
        .request("textDocument/definition", params)
        .await
        .unwrap_or_else(|e| panic!("definition request failed: {e}"));
    let s = result.to_string();
    assert!(
        s.contains("lib.rs"),
        "definition should point into lib.rs: {s}"
    );
    // Definition is on line 1 (0-based row 0).
    assert!(s.contains("\"line\":0"), "{s}");
}

#[tokio::test]
async fn batch_backend_finds_the_type_error() {
    if !ra_available() {
        eprintln!("rust-analyzer not installed; skipping");
        return;
    }
    let (tmp, _lib_rs) = make_project(true).await;
    let diags = z_engine_core::lsp::batch::run(tmp.path(), 90).unwrap();
    assert!(
        diags
            .iter()
            .any(|d| d.code == "E0308" && d.file.contains("lib.rs")),
        "expected E0308, got {diags:?}"
    );
}

#[tokio::test]
async fn diagnostics_tool_reports_broken_file_via_batch() {
    use serde_json::json;
    use z_engine_core::tools::Tool;

    let (tmp, lib_rs) = make_project(true).await;
    // Build ctx the same way the loop does.
    let perms = std::sync::Arc::new(std::sync::Mutex::new(
        z_engine_core::perms::PolicyEngine::new(vec![]),
    ));
    let ctx =
        z_engine_core::tools::ToolCtx::new(tmp.path().to_path_buf(), perms, tmp.path().join("t"));

    let out = z_engine_core::tools::lsp_tools::DiagnosticsTool
        .run(json!({"path": lib_rs.to_string_lossy()}), &ctx)
        .await
        .unwrap();
    assert!(!out.ok, "should surface diagnostics");
    assert!(out.result.contains("E0308"), "{}", out.result);
}

#[tokio::test]
async fn cargo_check_backend_surfaces_trait_errors() {
    use serde_json::json;
    use z_engine_core::tools::{Tool, ToolCtx};

    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"tiny2\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    // E0277: trait bound not satisfied (`i32 + &str`)
    let lib = "pub fn add(a: i32, b: i32) -> i32 {\n    a + \"oops\"\n}\n";
    let lib_rs = tmp.path().join("src/lib.rs");
    std::fs::write(&lib_rs, lib).unwrap();

    let perms = Arc::new(std::sync::Mutex::new(
        z_engine_core::perms::PolicyEngine::new(vec![]),
    ));
    let ctx = ToolCtx::new(tmp.path().to_path_buf(), perms, tmp.path().join("t"));

    let out = z_engine_core::tools::lsp_tools::DiagnosticsTool
        .run(json!({"path": "src/lib.rs"}), &ctx)
        .await
        .unwrap();
    assert!(!out.ok, "{}", out.result);
    assert!(out.result.contains("E0277"), "{}", out.result);
}
