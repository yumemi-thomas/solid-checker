//! A region per parsed file version, so a file's memory is returned when no
//! program of the chain reads it any more (safe code: no `unsafe`; the one
//! `unsafe_boundary` module decides when a region may be dropped).
//!
//! tsrs parses a program chain's files into the loader's per-thread regions
//! (`ProgramRegions`), which live as long as the chain. That keeps every
//! edited file's tree until the chain is replaced, and sends the data each
//! file fills lazily (line maps, name tables, identifier sets, the binder's
//! output when binding runs outside a region) to the thread arena, which is
//! never freed. A host that parses each file in a [`Region`] of its own, with
//! `bind_in_file_regions` on, puts all of it in the file's region: the same
//! scheme as tsrs's watch mode (`tsrs_execute` watchmem.rs).

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use tsrs_ast::{SourceFile, SourceFileParseOptions};
use tsrs_compiler::CompilerHost;
use tsrs_contentmapper::Mapper;
use tsrs_core::P;
use tsrs_core::arena::Region;
use tsrs_core::tspath::Path;
use tsrs_diagnostics::Message;
use tsrs_tsoptions::ParsedCommandLine;
use tsrs_vfs::FS;

/// The file regions of one program chain, by the address of the file they
/// hold, and the files its shared data refers to for as long as it lives.
#[derive(Default)]
pub struct FileRegions {
    regions: Mutex<HashMap<usize, Region>>,
    /// The files of the chain's full build: the processed-file data every
    /// program reused from it shares refers to them.
    base: Mutex<HashSet<usize>>,
}

impl FileRegions {
    /// A first chunk sized for the text, tree and binder output of a file
    /// (about eight times the text).
    fn first_chunk(text_len: usize) -> usize {
        text_len
            .saturating_mul(10)
            .saturating_add(4 << 10)
            .min(64 << 20)
    }

    fn parse(
        &self,
        size: usize,
        parse: impl FnOnce() -> Option<P<SourceFile>>,
    ) -> Option<P<SourceFile>> {
        let region = Region::new(Self::first_chunk(size));
        let file = {
            let _scope = region.enter();
            parse()
        }?;
        region.trim();
        // A host that returned a file it had parsed before keeps it where it is.
        if Region::containing(file.addr()).is_some_and(|owner| owner.ptr_eq(&region)) {
            self.lock().insert(file.addr(), region);
        }
        Some(file)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<usize, Region>> {
        self.regions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Records the files of the chain's full build.
    pub fn set_base(&self, files: impl Iterator<Item = usize>) {
        *self
            .base
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = files.collect();
    }

    /// Removes and returns the regions of the files that are neither a base
    /// file nor in `head`. The caller drops them once nothing that reads
    /// those files runs.
    pub fn take_dead(&self, head: impl Iterator<Item = usize>) -> Vec<Region> {
        let mut live: HashSet<usize> = head.collect();
        live.extend(
            self.base
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .copied(),
        );
        let mut regions = self.lock();
        if regions.len() <= live.len() {
            return Vec::new();
        }
        let dead: Vec<usize> = regions
            .keys()
            .copied()
            .filter(|address| !live.contains(address))
            .collect();
        dead.into_iter()
            .filter_map(|address| regions.remove(&address))
            .collect()
    }

    /// How many file regions are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The compiler host the program reads its files through: the given host,
/// with each parsed file in a region of the chain's [`FileRegions`].
pub struct RegionHost {
    inner: Arc<dyn CompilerHost>,
    files: Arc<FileRegions>,
}

impl RegionHost {
    pub fn wrap(inner: Arc<dyn CompilerHost>, files: &Arc<FileRegions>) -> Arc<dyn CompilerHost> {
        Arc::new(Self {
            inner,
            files: Arc::clone(files),
        })
    }
}

impl CompilerHost for RegionHost {
    fn fs(&self) -> &dyn FS {
        self.inner.fs()
    }
    fn default_library_path(&self) -> &str {
        self.inner.default_library_path()
    }
    fn get_current_directory(&self) -> &str {
        self.inner.get_current_directory()
    }
    fn trace(&self, msg: &'static Message, args: &[&dyn Display]) {
        self.inner.trace(msg, args);
    }
    fn get_source_file(&self, opts: SourceFileParseOptions) -> Option<P<SourceFile>> {
        let size = self
            .inner
            .fs()
            .stat(&opts.file_name)
            .map_or(0, |info| usize::try_from(info.size()).unwrap_or(0));
        self.files.parse(size, || self.inner.get_source_file(opts))
    }
    fn get_content_mapped_source_files(
        &self,
        parse_options: SourceFileParseOptions,
        mapper: &Mapper,
    ) -> Result<tsrs_contentmapper::SourceFiles, tsrs_contentmapper::Error> {
        self.inner
            .get_content_mapped_source_files(parse_options, mapper)
    }
    fn content_mapper_project(&self) -> Option<Arc<dyn tsrs_contentmapper::Project>> {
        self.inner.content_mapper_project()
    }
    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: Path,
    ) -> Option<P<ParsedCommandLine>> {
        self.inner.get_resolved_project_reference(file_name, path)
    }
}
