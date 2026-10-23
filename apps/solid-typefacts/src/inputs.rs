//! Digests of the program inputs a retained answer can depend on (ADR 0369).
//!
//! A retained answer records the digests it was computed under and is reused
//! only while every one is unchanged:
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
    /// Advances on every program change; a retained record validated at the
    /// current epoch needs no second check.
    epoch: u64,
    files: HashMap<String, FileInput>,
    by_path: Option<HashMap<String, P<SourceFile>>>,
    ambient: Option<Digest>,
    options: Option<Digest>,
    exports: Option<HashMap<String, Digest>>,
    /// Declaring paths interned for the dependency checks of retained
    /// answers, and each one's `(Text, Exports)` at an epoch.
    path_ids: HashMap<String, u32>,
    path_names: Vec<String>,
    declared: Vec<Option<(u64, Option<Digest>, Digest)>>,
    /// The names of the global scope's aliases (UMD exports, global
    /// `import =`), which scope lookup resolves to test their meaning.
    global_aliases: Option<std::sync::Arc<std::collections::HashSet<String>>>,
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
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The accepted program changed. `reused_except` names the one file a
    /// parse-reusing update replaced; `None` drops every text digest.
    pub fn program_changed(&mut self, reused_except: Option<&str>) {
        self.epoch += 1;
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
        self.global_aliases = None;
    }

    /// The names of the global scope's aliases in the accepted program (the
    /// checker's merged global table, read only).
    pub fn global_aliases(
        &mut self,
        checker: &tsrs_compiler::Checker,
    ) -> std::sync::Arc<std::collections::HashSet<String>> {
        std::sync::Arc::clone(self.global_aliases.get_or_insert_with(|| {
            let mut names = std::collections::HashSet::new();
            checker.globals.for_each(|name, symbol| {
                if symbol.flags().intersects(tsrs_ast::SymbolFlags::Alias) {
                    names.insert(name.to_owned());
                }
            });
            std::sync::Arc::new(names)
        }))
    }

    fn input(&mut self, file: P<SourceFile>) -> FileInput {
        *self
            .files
            .entry(clean(file.file_name()))
            .or_insert_with(|| file_input(file))
    }

    /// A small stable number for `path`, for [`ProgramInputs::declared`].
    pub fn path_id(&mut self, path: &str) -> u32 {
        if let Some(id) = self.path_ids.get(path) {
            return *id;
        }
        let id = u32::try_from(self.path_names.len()).expect("fewer than 2^32 paths");
        self.path_ids.insert(path.to_owned(), id);
        self.path_names.push(path.to_owned());
        self.declared.push(None);
        id
    }

    /// `(Text(path), Exports(path))` of an interned path, memoized for the
    /// epoch; the text is `None` when the program has no such file.
    pub fn declared(
        &mut self,
        program: &Program,
        exported: &HashMap<DurableRef, Preserved>,
        path: u32,
    ) -> (Option<Digest>, Digest) {
        let index = path as usize;
        if let Some((epoch, text, exports)) = self.declared[index]
            && epoch == self.epoch
        {
            return (text, exports);
        }
        let name = self.path_names[index].clone();
        let text = self.text_at(program, &name);
        let exports = self.exports(exported, &name);
        self.declared[index] = Some((self.epoch, text, exports));
        (text, exports)
    }

    /// `Text(file)`.
    pub fn text(&mut self, file: P<SourceFile>) -> Digest {
        self.input(file).text
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
