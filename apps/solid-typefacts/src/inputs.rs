//! Digests of the program inputs a kept pass output can depend on (`passes`).
//!
//! A kept pass is reused only while every digest it depends on is unchanged:
//!
//! * `Text(path)`: the file's exact bytes;
//! * `Exports(path)`: which declaration references of `path` the program's
//!   modules export (the scan behind `Export` keys, ADR 0366), so a symbol's
//!   key kind is the same;
//! * `Ambient`: every file that adds to the global scope or augments a module
//!   (scripts, lib files, `declare global`, `declare module "m"` inside a
//!   module, `export as namespace`), with its text, in program order;
//! * `Options`: the compiler options.
//!
//! Digests are recomputed for the accepted program and memoized until the next
//! program change. A text digest survives an update only when the update
//! reused every other file's parse (`UpdateKind::Reused` replaces exactly the
//! edited file); any rebuild re-reads every file and clears them all.

use crate::durable::DurableRef;
use crate::gocompat::clean;
use crate::identity::Preserved;
use crate::shape::has_global_or_module_augmentation;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use tsrs_ast::{Kind, SourceFile, is_external_module};
use tsrs_compiler::Program;
use tsrs_core::P;

pub type Digest = [u8; 32];

#[derive(Clone, Copy)]
struct FileInput {
    text: Digest,
    /// The file adds to the global scope or augments a module.
    ambient: bool,
}

#[derive(Default)]
pub struct ProgramInputs {
    files: HashMap<String, FileInput>,
    by_path: Option<HashMap<String, P<SourceFile>>>,
    ambient: Option<Digest>,
    options: Option<Digest>,
    exports: Option<HashMap<String, Digest>>,
}

fn file_input(file: P<SourceFile>) -> FileInput {
    FileInput {
        text: Sha256::digest(file.text().as_bytes()).into(),
        ambient: contributes_to_global_scope(file),
    }
}

/// A script (its declarations are globals), or a module that declares
/// `global`, augments a module, or exports a UMD global.
fn contributes_to_global_scope(file: P<SourceFile>) -> bool {
    if !is_external_module(file) {
        return true;
    }
    has_global_or_module_augmentation(file)
        || file
            .statements()
            .nodes()
            .iter()
            .any(|statement| statement.kind() == Kind::NamespaceExportDeclaration)
}

impl ProgramInputs {
    /// The accepted program changed. `reused_except` names the one file a
    /// parse-reusing update replaced; `None` drops every text digest.
    pub fn program_changed(&mut self, reused_except: Option<&str>) {
        match reused_except {
            Some(path) => {
                self.files.remove(&clean(path));
            }
            None => self.files.clear(),
        }
        self.by_path = None;
        self.ambient = None;
        self.options = None;
        self.exports = None;
    }

    fn input(&mut self, file: P<SourceFile>) -> FileInput {
        *self
            .files
            .entry(clean(file.file_name()))
            .or_insert_with(|| file_input(file))
    }

    /// `Text(file)`.
    pub fn text(&mut self, file: P<SourceFile>) -> Digest {
        self.input(file).text
    }

    /// Whether `file` adds to the global scope or augments a module (a
    /// member of `Ambient`).
    pub fn is_ambient(&mut self, file: P<SourceFile>) -> bool {
        self.input(file).ambient
    }

    /// `Text(path)`, or `None` when the program has no such file.
    pub fn text_at(&mut self, program: &Program, path: &str) -> Option<Digest> {
        if let Some(input) = self.files.get(path) {
            return Some(input.text);
        }
        let file = *self
            .by_path
            .get_or_insert_with(|| {
                program
                    .source_files()
                    .iter()
                    .map(|file| (clean(file.file_name()), *file))
                    .collect()
            })
            .get(path)?;
        Some(self.text(file))
    }

    /// `Ambient`.
    pub fn ambient(&mut self, program: &Program) -> Digest {
        if let Some(digest) = self.ambient {
            return digest;
        }
        let mut hasher = Sha256::new();
        for file in program.source_files() {
            let input = self.input(*file);
            if input.ambient {
                hasher.update(clean(file.file_name()).as_bytes());
                hasher.update([0]);
                hasher.update(input.text);
            }
        }
        let digest: Digest = hasher.finalize().into();
        self.ambient = Some(digest);
        digest
    }

    /// `Options`.
    pub fn options(&mut self, program: &Program) -> Digest {
        *self.options.get_or_insert_with(|| {
            Sha256::digest(format!("{:?}", *program.options()).as_bytes()).into()
        })
    }

    /// `Exports(path)`: the exported declaration references declared in
    /// `path`, from the exports scan of the accepted program.
    pub fn exports(&mut self, exported: &HashMap<DurableRef, Preserved>, path: &str) -> Digest {
        let digests = self.exports.get_or_insert_with(|| {
            let mut by_path: HashMap<&str, Vec<&DurableRef>> = HashMap::new();
            for reference in exported.keys() {
                by_path.entry(&reference.path).or_default().push(reference);
            }
            by_path
                .into_iter()
                .map(|(path, mut references)| {
                    references.sort_by(|left, right| {
                        (left.start_byte, left.end_byte, &left.name).cmp(&(
                            right.start_byte,
                            right.end_byte,
                            &right.name,
                        ))
                    });
                    let mut hasher = Sha256::new();
                    for reference in references {
                        hasher.update(reference.start_byte.to_le_bytes());
                        hasher.update(reference.end_byte.to_le_bytes());
                        hasher.update(reference.name.as_bytes());
                        hasher.update([0]);
                    }
                    (path.to_owned(), hasher.finalize().into())
                })
                .collect()
        });
        digests.get(path).copied().unwrap_or_default()
    }
}
