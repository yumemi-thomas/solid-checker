//! Fixtures for the tests that run the real compiler: a scratch directory
//! that cleans up after itself, and an opened [`Project`] over it.

use crate::context::Context;
use crate::project::Project;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use typefacts::v3::FileChange;

pub(crate) struct Scratch {
    pub root: PathBuf,
}

static NEXT: AtomicU64 = AtomicU64::new(0);

impl Scratch {
    /// A fresh directory (realpath-resolved: macOS hands out a symlinked
    /// temporary root, and the resolver takes a realpath of its own for
    /// node_modules resolutions, so an unresolved root would make every
    /// external import look symlinked and would hide the one case that
    /// genuinely is) holding `files`.
    pub fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "solid-typefacts-rs-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let scratch = Self {
            root: root.canonicalize().unwrap(),
        };
        scratch.write_all(files);
        scratch
    }

    pub fn write_all(&self, files: &[(&str, &str)]) {
        for (name, contents) in files {
            let path = self.root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }

    /// The absolute path of `name` under the scratch root.
    pub fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    pub fn config(&self) -> String {
        self.path("tsconfig.json")
    }

    pub fn open(&self) -> Project {
        Project::open(&Context::new(), &self.config(), None, None).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(crate) fn change(path: &str, version: u64, source: &str) -> FileChange {
    FileChange {
        path: path.to_owned(),
        source: source.as_bytes().to_vec(),
        deleted: false,
        version,
    }
}

pub(crate) fn deletion(path: &str, version: u64) -> FileChange {
    FileChange {
        path: path.to_owned(),
        source: Vec::new(),
        deleted: true,
        version,
    }
}

pub(crate) const BUNDLER_CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "module": "esnext",
    "target": "esnext",
    "moduleResolution": "bundler",
    "allowJs": true
  },
  "include": ["**/*.ts", "**/*.js", "**/*.d.ts"]
}"#;

pub(crate) const SMALL_CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#;

pub(crate) fn relative(root: &Path, path: &str) -> Option<String> {
    Path::new(path)
        .strip_prefix(root)
        .ok()
        .map(|rest| rest.to_string_lossy().replace('\\', "/"))
}
