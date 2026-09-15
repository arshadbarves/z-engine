use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct TestWorkspace(pub PathBuf);

impl TestWorkspace {
    pub fn new() -> Self {
        let root = std::env::current_dir()
            .unwrap()
            .join(".z-engine/verification-tests")
            .join(ulid::Ulid::new().to_string());
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("Cargo.toml"),
            "[package]\nname = \"verification-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n").unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        Self(root.canonicalize().unwrap())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, "verification test fixture cleanup failed");
        }
    }
}
