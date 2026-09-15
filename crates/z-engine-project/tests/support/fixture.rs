use std::path::{Path, PathBuf};

use z_engine_project::{DiscoveryOptions, ProjectReport, discover};

pub struct Fixture(tempfile::TempDir);

impl Fixture {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let fixture = Self(tempfile::tempdir().unwrap());
        for (name, text) in files {
            fixture.write(name, text);
        }
        fixture
    }

    pub fn write(&self, name: &str, text: &str) {
        let path = self.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }

    pub fn root(&self) -> PathBuf {
        self.path().canonicalize().unwrap()
    }

    pub fn discover(&self) -> ProjectReport {
        discover(&self.root(), &DiscoveryOptions::default()).unwrap()
    }
}
