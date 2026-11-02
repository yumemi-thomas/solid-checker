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

    /// Parses into a region of its own; returns the file and, when the file
    /// is in that region (the chain now holds it), the region.
    fn parse(
        &self,
        size: usize,
        parse: impl FnOnce() -> Option<P<SourceFile>>,
    ) -> Option<(P<SourceFile>, Option<Region>)> {
        let region = Region::new(Self::first_chunk(size));
        let file = {
            let _scope = region.enter();
            parse()
        }?;
        region.trim();
        // A host that returned a file it had parsed before keeps it where it is.
        if Region::containing(file.addr()).is_some_and(|owner| owner.ptr_eq(&region)) {
            self.lock().insert(file.addr(), region.clone());
            return Some((file, Some(region)));
        }
        Some((file, None))
    }

    /// Holds `region`, which holds `file`, for as long as this chain lives: a
    /// file another chain parsed ([`ParseCache`]).
    fn adopt(&self, file: P<SourceFile>, region: Region) {
        self.lock().entry(file.addr()).or_insert(region);
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

/// The parsed files of a project's chains, for the next chain to take instead
/// of parsing them again.
///
/// An update whose edited file cannot be replaced in place (its imports
/// moved) builds a fresh chain, which would parse every file of the program
/// again: the bundled libraries and the installed declarations too, about
/// two thirds of such an update. A parse is a function of the parse options
/// and the text alone, and a bound file is frozen (`unsafe_boundary`
/// invariant 8), so a file whose options are equal and whose text is
/// byte-equal to what the host reads now is the file a new parse would make.
/// tsrs shares files across chains the same way: its watch mode across full
/// builds (`tsrs_execute` watcher.rs, keyed by path and modification time),
/// its project system across programs (`tsrs_project` parsecache.rs, keyed by
/// the options and a content hash). Here the key is the options and the text
/// itself is compared, so nothing is trusted that a fresh parse could answer
/// differently.
///
/// A chain that takes a file adopts its region ([`FileRegions::adopt`]), so
/// the file lives as long as any chain reads it, whatever the cache does
/// later; the cache's own handle goes when a commit leaves the file out of
/// the head ([`ParseCache::retain`]).
#[derive(Default)]
pub struct ParseCache {
    entries: Mutex<HashMap<SourceFileParseOptions, (P<SourceFile>, Region)>>,
    /// Each held file's text digest once computed (`inputs::ProgramInputs`),
    /// by the file's address: unique while the entry holds the file's region,
    /// and dropped with the entry, so an address never names another file
    /// here.
    digests: Mutex<HashMap<usize, Option<[u8; 32]>>>,
    hits: std::sync::atomic::AtomicU64,
}

impl ParseCache {
    fn lock(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<SourceFileParseOptions, (P<SourceFile>, Region)>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The file parsed with `options` from exactly the text `fs` holds for
    /// it now, and its region.
    fn lookup(
        &self,
        options: &SourceFileParseOptions,
        fs: &dyn FS,
    ) -> Option<(P<SourceFile>, Region)> {
        let (file, region) = self.lock().get(options).cloned()?;
        // The region is held, so the text is readable outside the lock.
        let current = current_text(fs, &options.file_name)?;
        (file.text() == current.as_ref()).then(|| {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (file, region)
        })
    }

    fn digests(&self) -> std::sync::MutexGuard<'_, HashMap<usize, Option<[u8; 32]>>> {
        self.digests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn store(&self, options: SourceFileParseOptions, file: P<SourceFile>, region: Region) {
        // The digests lock is taken inside the entries lock, here and in
        // `retain`, so an entry and its digest slot come and go together.
        let mut entries = self.lock();
        let mut digests = self.digests();
        if let Some((replaced, _)) = entries.insert(options, (file, region)) {
            digests.remove(&replaced.addr());
        }
        digests.insert(file.addr(), None);
    }

    /// Keeps the entries of the files in `head` (a committed head's files):
    /// the others are no chain's to take next, and every chain that read one
    /// holds its region itself.
    pub fn retain(&self, head: impl Iterator<Item = usize>) {
        let head: HashSet<usize> = head.collect();
        let mut entries = self.lock();
        let mut digests = self.digests();
        entries.retain(|_, (file, _)| head.contains(&file.addr()));
        digests.retain(|address, _| head.contains(address));
    }

    /// The digest of `file`'s text, computed by `digest` the first time for
    /// a file this cache holds, and every time for another.
    pub fn text_digest(&self, file: P<SourceFile>, digest: impl FnOnce() -> [u8; 32]) -> [u8; 32] {
        if let Some(Some(known)) = self.digests().get(&file.addr()) {
            return *known;
        }
        let computed = digest();
        if let Some(slot) = self.digests().get_mut(&file.addr()) {
            *slot = Some(computed);
        }
        computed
    }

    /// The files a chain took from the cache so far.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many parsed files the cache holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
thread_local! {
    static PARSE_CACHE_OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a project opened now on this thread keeps a [`ParseCache`]: always,
/// except in a test's oracle ([`without_parse_cache`]).
#[must_use]
pub fn parse_cache_enabled() -> bool {
    #[cfg(test)]
    {
        !PARSE_CACHE_OFF.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// Runs `open` with the projects it opens on this thread parsing every chain
/// afresh: the oracle a cached project is compared with.
#[cfg(test)]
pub fn without_parse_cache<R>(open: impl FnOnce() -> R) -> R {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            PARSE_CACHE_OFF.with(|off| off.set(false));
        }
    }
    PARSE_CACHE_OFF.with(|off| off.set(true));
    let _restore = Restore;
    open()
}

/// The text a host would parse for `file_name` now: a bundled library's
/// embedded text, else what the file system reads.
fn current_text(fs: &dyn FS, file_name: &str) -> Option<std::borrow::Cow<'static, str>> {
    if let Some(text) = tsrs_vfs::bundled::embedded_file(file_name) {
        return Some(std::borrow::Cow::Borrowed(text));
    }
    fs.read_file(file_name).map(std::borrow::Cow::Owned)
}

/// The compiler host the program reads its files through: the given host,
/// with each parsed file in a region of the chain's [`FileRegions`], and with
/// the project's [`ParseCache`] when it has one.
pub struct RegionHost {
    inner: Arc<dyn CompilerHost>,
    files: Arc<FileRegions>,
    cache: Option<Arc<ParseCache>>,
}

impl RegionHost {
    pub fn wrap(inner: Arc<dyn CompilerHost>, files: &Arc<FileRegions>) -> Arc<dyn CompilerHost> {
        Self::wrap_cached(inner, files, None)
    }

    pub fn wrap_cached(
        inner: Arc<dyn CompilerHost>,
        files: &Arc<FileRegions>,
        cache: Option<&Arc<ParseCache>>,
    ) -> Arc<dyn CompilerHost> {
        Arc::new(Self {
            inner,
            files: Arc::clone(files),
            cache: cache.map(Arc::clone),
        })
    }
}

impl RegionHost {
    fn parse(&self, opts: SourceFileParseOptions) -> Option<(P<SourceFile>, Option<Region>)> {
        let size = self
            .inner
            .fs()
            .stat(&opts.file_name)
            .map_or(0, |info| usize::try_from(info.size()).unwrap_or(0));
        self.files.parse(size, || self.inner.get_source_file(opts))
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
        let Some(cache) = &self.cache else {
            return self.parse(opts).map(|(file, _)| file);
        };
        if let Some((file, region)) = cache.lookup(&opts, self.inner.fs()) {
            self.files.adopt(file, region);
            return Some(file);
        }
        let (file, region) = self.parse(opts.clone())?;
        // Its text is the file's own: the next lookup compares it with the
        // disk then.
        if let Some(region) = region {
            cache.store(opts, file, region);
        }
        Some(file)
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
