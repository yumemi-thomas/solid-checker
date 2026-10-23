//! The only module of this crate that uses `unsafe` (repo policy, AGENTS.md;
//! the crate otherwise denies `unsafe_code`).
//!
//! # Why there is `unsafe` at all
//!
//! tsrs leaks by design: `new_program` returns `&'static Program`, every AST
//! node, symbol and type lives in a thread-local arena, and nothing is given
//! back unless the embedder owns the lifecycle. A producer that serves a
//! project for hours, rebuilding and updating it, must give memory back. tsrs
//! offers that as `unsafe` frees (`free_program`, `SharedProgramData::free`)
//! and `CheckerHandle::from_raw` for an externally owned checker. This module
//! wraps all of them behind [`Generation`], a safe RAII type: no other module
//! frees, forgets or fabricates a tsrs lifetime.
//!
//! # What a [`Generation`] owns
//!
//! One program chain head:
//!
//! * the `&'static Program` (made by `new_program`, or by `reuse_program` from
//!   the previous head, which shares the previous head's processed-file data);
//! * the [`ProgramRegions`] the file loader allocated the chain's parsed files
//!   in (one set per full build, shared by every head reused from it);
//! * the chain's [`FileRegions`]: every parsed file version has a region of
//!   its own (parse, bind and the data the file fills lazily), and
//!   [`Generation::commit`] frees the versions no head reads any more (the
//!   full build's files stay: the shared processed-file data refers to them);
//! * the region the parsed configuration lives in, shared by every head and
//!   chain made from it, so the configuration is freed with the last of them;
//! * the program's single checker pool (Go: `singleCheckerPool`, ADR 0004),
//!   whose checker lives in a [`Region`] of its own so disposing it returns
//!   its memory;
//! * whether it owns the chain's shared data (`owns_shared`).
//!
//! # Invariants (the contract every caller must keep)
//!
//! 1. **Nothing holding a `P<T>` / `&'static` from a generation may outlive
//!    it.** `P<SourceFile>`, `P<Symbol>`, `P<Node>`, `&'static Program` are
//!    plain copyable handles: the type system cannot tie them to the
//!    generation they came from, because tsrs's API hands out `'static`.
//!    Callers therefore copy results out (strings, integers, owned structs)
//!    before the generation is dropped, replaced, or its checker is released.
//! 2. **Checkers do not escape [`Generation::with_checker`].** A
//!    `CheckerHandle` is a lease: the pool counts outstanding leases and a
//!    generation with an outstanding lease is *leaked instead of freed*
//!    (never freed under a live reference; the leak is the fail-safe).
//! 3. **Single worker thread.** tsrs arenas are thread-local. A generation
//!    records the thread that built it and is only ever freed there; dropped
//!    elsewhere it is leaked.
//! 4. **Order of disposal** (the order `tsrs_project::memregions` and
//!    `tsrs_execute::watcher` use): the checker, then its region; then the
//!    program (`free_program`); then, if this head owns the chain's shared
//!    data, `SharedProgramData::free`; then the chain's file regions, the
//!    loader's [`ProgramRegions`] and the configuration's region, in that
//!    order (the fields' declaration order), each when its last `Arc` drops,
//!    which is after everything that points into them is gone.
//! 5. **Shared data is freed exactly once**, by the head that owns it. A
//!    head made by [`Generation::prepare_update`] from a reusable previous
//!    head does not own it until [`Generation::commit`] transfers ownership,
//!    so an update that is abandoned frees only its own program.
//! 6. Diagnostics point into checker regions; callers copy `code/pos/end`
//!    before the checker is released.
//! 7. **No tsrs entry point that acquires a checker may run while a lease
//!    is held.** `Program::get_semantic_diagnostics`,
//!    `get_type_checker_for_file_exclusive` and every other program-level call
//!    that reaches the pool acquires the checker exclusively; a second
//!    acquisition would hand out a second `&mut Checker` to the same checker.
//!    The pool therefore **panics** on an acquisition while a lease is
//!    outstanding (all builds: the alternative is aliased `&mut`). A caller
//!    that holds a [`CheckerLease`] and needs such a call (the call-validity
//!    fallback needs a file's semantic diagnostics) uses
//!    [`CheckerLease::semantic_diagnostics`], which releases the lease, makes
//!    the call and reacquires. `P<T>` handles taken before the suspension stay
//!    valid: the checker and its region belong to the slot, not to the lease.
//!    While suspended the lease cannot be dereferenced (it panics), and the
//!    borrow checker already forbids any `&mut Checker` derived from it from
//!    living across the call.
//!
//! Violations of 2 and 3 degrade to a leak. Violations of 1 and 6 cannot be
//! detected here; the S3 suite runs under `TSRS_ARENA_POISON=1` so a read of
//! freed arena memory is poisoned instead of silently stale.
#![allow(unsafe_code)]

use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use tsrs_ast::SourceFile;
use tsrs_compiler::{
    Checker, CheckerHandle, CheckerPool, CompilerHost, Context, CreateCheckerPool, PooledChecker,
    Program, ProgramOptions, ProgramRegions, free_program, new_program, shared_program_data,
};
use tsrs_core::arena::Region;
use tsrs_core::{P, Tristate};
use tsrs_tsoptions::ParsedCommandLine;

use crate::file_regions::{FileRegions, RegionHost};

/// A value the configuration parser borrows for `'static` (its module resolver
/// requires a `&'static` host), freed with its owner instead of leaked.
///
/// The reference [`HostLease::as_static`] returns is `'static` only because the
/// parser's signature says so. The contract is the module's invariant 1 applied
/// to it: the parser keeps the reference only while a configuration is parsed
/// (`ParsedCommandLine` does not retain it), so the owner holds the lease
/// beside the generations it parsed for and declares it after them, which drops
/// the lease after every configuration parsed through it.
pub struct HostLease<T: Send + Sync + 'static> {
    pointer: NonNull<T>,
}

// SAFETY: the lease owns a `Box<T>`; `T: Send + Sync` makes moving it between
// threads and sharing the reference it hands out sound.
unsafe impl<T: Send + Sync + 'static> Send for HostLease<T> {}
// SAFETY: as above; `as_static` hands out shared references only.
unsafe impl<T: Send + Sync + 'static> Sync for HostLease<T> {}

impl<T: Send + Sync + 'static> HostLease<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            pointer: NonNull::from(Box::leak(Box::new(value))),
        }
    }

    /// The value, as the parser wants it. Must not be stored past the lease.
    #[must_use]
    pub fn as_static(&self) -> &'static T {
        // SAFETY: the pointer came from a leaked `Box` and stays valid until
        // `drop`; the caller's contract (above) is that nothing keeps the
        // reference past the lease.
        unsafe { self.pointer.as_ref() }
    }
}

impl<T: Send + Sync + 'static> Drop for HostLease<T> {
    fn drop(&mut self) {
        // SAFETY: made by `Box::leak` in `new` and freed only here; the owner
        // dropped every configuration parsed through it first (above).
        drop(unsafe { Box::from_raw(self.pointer.as_ptr()) });
    }
}

/// The program's single checker (Go: `singleCheckerPool`). The checker is
/// created on first use inside a [`Region`] of its own, so disposing it gives
/// its memory back without touching the program.
struct CheckerSlot {
    program: &'static Program,
    held: Mutex<Option<Held>>,
    /// Outstanding [`CheckerHandle`]s handed out by `get_checker`.
    leases: AtomicUsize,
}

/// Field order is drop order: the checker goes before the region it lives in.
struct Held {
    checker: PooledChecker,
    _region: Region,
}

impl CheckerSlot {
    fn new(program: &'static Program) -> Arc<Self> {
        Arc::new(Self {
            program,
            held: Mutex::new(None),
            leases: AtomicUsize::new(0),
        })
    }

    fn outstanding(&self) -> usize {
        self.leases.load(Ordering::SeqCst)
    }

    /// Drops the checker and its region. Refuses (returns `false`, drops
    /// nothing) while a lease is outstanding.
    fn dispose(&self) -> bool {
        if self.outstanding() != 0 {
            return false;
        }
        let held = self
            .held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(held);
        true
    }
}

/// The `CheckerPool` the program holds. It is exclusive in practice (one
/// worker thread, no re-entrant acquisition), like Go's pool whose
/// file-affine lease is non-exclusive because every entry point is already
/// serialized.
struct SlotPool(Arc<CheckerSlot>);

impl CheckerPool for SlotPool {
    fn get_checker(&self, _ctx: &Context, _file: Option<P<SourceFile>>) -> CheckerHandle {
        let slot = &self.0;
        // Module invariant 7: exclusive acquisition. A second handle would be a
        // second `&mut Checker` to the one checker.
        assert!(
            slot.outstanding() == 0,
            "a tsrs entry point acquired the checker while a lease was held \
             (unsafe_boundary invariant 7)"
        );
        let mut guard = slot
            .held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_none() {
            let region = Region::new(1 << 20);
            let checker = {
                let _scope = region.enter();
                PooledChecker::new(tsrs_checker::new_checker(slot.program))
            };
            *guard = Some(Held {
                checker,
                _region: region,
            });
        }
        let held = guard.as_mut().expect("the checker was just created");
        let pointer: NonNull<Checker> = held.checker.as_non_null();
        let scope = {
            let region = &held._region;
            region.enter()
        };
        slot.leases.fetch_add(1, Ordering::SeqCst);
        let slot = Arc::clone(&self.0);
        // SAFETY: `from_raw` needs the checker to stay alive and untouched by
        // anyone else until the release closure runs. The slot owns it in
        // `held` and only `CheckerSlot::dispose` (which refuses while
        // `leases != 0`) or the slot's own drop frees it; the lease count is
        // decremented by the release closure, after the region scope ends.
        // The slot is kept alive by the `Arc` the closure owns. Exclusivity:
        // callers hold at most one handle at a time and all use is on the one
        // worker thread (module invariants 2 and 3).
        unsafe {
            CheckerHandle::from_raw(pointer, move || {
                drop(scope);
                slot.leases.fetch_sub(1, Ordering::SeqCst);
            })
        }
    }
}

/// One semantic diagnostic, copied out of the checker's regions (invariant 6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawDiagnostic {
    pub code: i32,
    /// `None` for a diagnostic without a source position (Go: `-1`).
    pub pos: Option<u32>,
    pub end: Option<u32>,
}

fn diagnostics_of(program: &'static Program, file: P<SourceFile>) -> Vec<RawDiagnostic> {
    program
        .get_semantic_diagnostics(&Context::default(), Some(file))
        .into_iter()
        .map(|diagnostic| RawDiagnostic {
            code: diagnostic.code(),
            pos: diagnostic.pos(),
            end: diagnostic.end(),
        })
        .collect()
}

/// The program's checker, leased to the caller (module invariants 2 and 7).
/// Dereferences to the [`Checker`]; the lease can be suspended around a tsrs
/// call that acquires the checker itself.
pub struct CheckerLease {
    program: &'static Program,
    /// `None` only while [`CheckerLease::semantic_diagnostics`] runs.
    handle: Option<CheckerHandle>,
}

impl CheckerLease {
    fn acquire(program: &'static Program) -> Self {
        Self {
            program,
            handle: Some(program.get_type_checker(&Context::default())),
        }
    }

    /// The semantic diagnostics of `file`, as the compiler reports them for the
    /// program's single checker (Go: `Program.GetSemanticDiagnostics`). The
    /// lease is released for the call and reacquired after it (module
    /// invariant 7); the checker is the same one, so types and symbols taken
    /// before stay valid.
    pub fn semantic_diagnostics(&mut self, file: P<SourceFile>) -> Vec<RawDiagnostic> {
        drop(self.handle.take());
        let diagnostics = diagnostics_of(self.program, file);
        self.handle = Some(self.program.get_type_checker(&Context::default()));
        diagnostics
    }
}

impl std::ops::Deref for CheckerLease {
    type Target = Checker;
    fn deref(&self) -> &Checker {
        self.handle.as_deref().expect("the lease is not suspended")
    }
}

impl std::ops::DerefMut for CheckerLease {
    fn deref_mut(&mut self) -> &mut Checker {
        self.handle
            .as_deref_mut()
            .expect("the lease is not suspended")
    }
}

/// How a head relates to the previous one when [`Generation::prepare_update`]
/// made it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateKind {
    /// `reuse_program` replaced one file in place and shares the previous
    /// head's processed-file data.
    Reused,
    /// The file could not be replaced in place; a fresh program chain was
    /// built from the same parsed configuration (Go: `UpdateProgram`'s
    /// internal `NewProgram` fallback).
    Rebuilt,
}

/// One live program: see the module docs for what it owns and the contract.
pub struct Generation {
    program: &'static Program,
    slot: Arc<CheckerSlot>,
    /// The chain's file regions. Declared before `regions`: a file region
    /// goes before the loader's regions, whose data points at the files.
    files: Arc<FileRegions>,
    regions: Arc<ProgramRegions>,
    /// The region the chain's parsed configuration lives in. Last: the
    /// program and its data read the configuration until they are freed.
    config: Region,
    owns_shared: bool,
    updates: u32,
    thread: ThreadId,
    kind: UpdateKind,
}

fn options_for(
    config: P<ParsedCommandLine>,
    host: Arc<dyn CompilerHost>,
    regions: &Arc<ProgramRegions>,
    factory: &CreateCheckerPool,
) -> ProgramOptions {
    // tsrs's canonical checker history (ADR 0367): the caches typescript-go
    // keys by what the checker saw first (`undefinedProperties`, keyed by name
    // across the program, orders the properties of a normalized object-literal
    // union by the first literal that mentioned the name) are a function of the
    // program alone. Printed text can still follow type creation order (union
    // constituents); no decision reads it. A process-wide flag that has to be
    // set before any checker exists, and the same value on every call.
    tsrs_core::compat::set_go_compatible_history(false);
    // Binding runs in the region of the file it binds (and a file's lazily
    // filled data goes there too) once files have regions of their own
    // (`RegionHost`). A process-wide flag, set before any file is parsed.
    tsrs_compiler::bind_in_file_regions();
    let mut options = ProgramOptions::new(config, host);
    // Parse and bind in parallel but keep exactly one checker (ADR 0004):
    // the external pool is what keeps both properties.
    options.single_threaded = Tristate::False;
    options.use_source_of_project_reference = true;
    options.create_checker_pool = Some(Arc::clone(factory));
    // Memory: files are allocated in `regions`, and a program reused from
    // this one allocates its clones in a region the program frees.
    options.checker_regions = true;
    options.checker_diagnostic_regions = true;
    options.regions = Some(Arc::clone(regions));
    // One checker reports diagnostics (S4 reads them for call validity).
    options.canonical_diagnostics = false;
    options
}

/// A pool factory that remembers the slot it made for the program it was
/// given, so the generation can reach the slot later.
fn slot_factory() -> (CreateCheckerPool, Arc<Mutex<Option<Arc<CheckerSlot>>>>) {
    let cell: Arc<Mutex<Option<Arc<CheckerSlot>>>> = Arc::new(Mutex::new(None));
    let writer = Arc::clone(&cell);
    let factory: CreateCheckerPool = Arc::new(move |program: &'static Program| {
        let slot = CheckerSlot::new(program);
        *writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&slot));
        Box::new(SlotPool(slot)) as Box<dyn CheckerPool>
    });
    (factory, cell)
}

fn take_slot(cell: &Mutex<Option<Arc<CheckerSlot>>>) -> Arc<CheckerSlot> {
    cell.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("the program constructor created its checker pool")
}

impl Generation {
    /// Builds a program chain from a parsed configuration and binds it (Go:
    /// `buildProgram` after the configuration parsed, plus `BindSourceFiles`).
    ///
    /// `config` was parsed, and `config_region` was the allocation target while
    /// it was (so freeing the generation frees the configuration, and what its
    /// vectors and strings own, with it).
    pub fn build(
        config_region: Region,
        config: P<ParsedCommandLine>,
        host: Arc<dyn CompilerHost>,
    ) -> Generation {
        let regions = Arc::new(ProgramRegions::default());
        let files = Arc::new(FileRegions::default());
        let host = RegionHost::wrap(host, &files);
        let (factory, cell) = slot_factory();
        let program = new_program(options_for(config, host, &regions, &factory));
        program.bind_source_files();
        files.set_base(program.source_files().iter().map(|file| file.addr()));
        Generation {
            program,
            slot: take_slot(&cell),
            files,
            regions,
            config: config_region,
            owns_shared: true,
            updates: 0,
            thread: std::thread::current().id(),
            kind: UpdateKind::Rebuilt,
        }
    }

    /// A fresh program chain from this generation's configuration (the rebase
    /// policy). The configuration, and its region, are shared with this
    /// generation, so replacing it frees only the old chain.
    #[must_use]
    pub fn rebuilt(&self, host: Arc<dyn CompilerHost>) -> Generation {
        self.fresh_chain(host)
    }

    /// The parsed configuration this program was built from.
    #[must_use]
    pub fn command_line(&self) -> P<ParsedCommandLine> {
        self.program.command_line()
    }

    /// The program. `'static` is a tsrs artifact: the reference is valid until
    /// this generation is dropped or replaced, and must not be stored past
    /// that (module invariant 1).
    #[must_use]
    pub fn program(&self) -> &'static Program {
        self.program
    }

    /// Updates since the last full build of this chain.
    #[must_use]
    pub fn updates_since_build(&self) -> u32 {
        self.updates
    }

    /// How this generation was made by `prepare_update`.
    #[must_use]
    pub fn kind(&self) -> UpdateKind {
        self.kind
    }

    /// Runs `f` with the program's checker, creating it on first use. The
    /// lease ends when `f` returns (module invariant 2).
    pub fn with_checker<R>(&self, f: impl FnOnce(&mut CheckerLease) -> R) -> R {
        let mut lease = CheckerLease::acquire(self.program);
        f(&mut lease)
    }

    /// The semantic diagnostics of `file` with no lease held (module
    /// invariant 7; the pool panics if one is).
    #[must_use]
    pub fn semantic_diagnostics(&self, file: P<SourceFile>) -> Vec<RawDiagnostic> {
        diagnostics_of(self.program, file)
    }

    /// Drops the checker and everything it allocated (Go:
    /// `ReleaseAnalysisState`). Returns `false`, changing nothing, if a lease
    /// is somehow outstanding.
    pub fn release_checker(&mut self) -> bool {
        self.slot.dispose()
    }

    /// Whether the program holds a checker now.
    #[must_use]
    pub fn has_checker(&self) -> bool {
        self.slot
            .held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// Makes the head that replaces `changed_path`'s file, leaving `self`
    /// intact. `host` serves the candidate's files. The candidate is bound.
    /// Dropping it without [`Generation::commit`] frees only what it
    /// allocated.
    ///
    /// # Panics
    ///
    /// If `changed_path` is not a file of this program: callers check first.
    #[must_use]
    pub fn prepare_update(&self, changed_path: &str, host: Arc<dyn CompilerHost>) -> Generation {
        let old_file = self
            .program
            .get_source_file(changed_path)
            .expect("the changed file is in the program");
        let (factory, cell) = slot_factory();
        let (reused, _new_file, _) = self.program.reuse_program(
            old_file.path(),
            RegionHost::wrap(Arc::clone(&host), &self.files),
            Some(Arc::clone(&factory)),
            None,
        );
        if let Some(program) = reused {
            program.bind_source_files();
            return Generation {
                program,
                slot: take_slot(&cell),
                files: Arc::clone(&self.files),
                regions: Arc::clone(&self.regions),
                config: self.config.clone(),
                owns_shared: false,
                updates: self.updates + 1,
                thread: self.thread,
                kind: UpdateKind::Reused,
            };
        }
        // A fresh chain from the same parsed configuration, in regions of its
        // own, so replacing this head frees the whole old chain.
        self.fresh_chain(host)
    }

    /// A new program chain over this generation's configuration, in regions of
    /// its own.
    fn fresh_chain(&self, host: Arc<dyn CompilerHost>) -> Generation {
        let (factory, cell) = slot_factory();
        let regions = Arc::new(ProgramRegions::default());
        let files = Arc::new(FileRegions::default());
        let program = new_program(options_for(
            self.program.command_line(),
            RegionHost::wrap(host, &files),
            &regions,
            &factory,
        ));
        program.bind_source_files();
        files.set_base(program.source_files().iter().map(|file| file.addr()));
        Generation {
            program,
            slot: take_slot(&cell),
            files,
            regions,
            config: self.config.clone(),
            owns_shared: true,
            updates: 0,
            thread: self.thread,
            kind: UpdateKind::Rebuilt,
        }
    }

    /// Installs `candidate` (from [`Generation::prepare_update`] or
    /// [`Generation::build`]) as this generation and frees the previous head.
    pub fn commit(&mut self, mut candidate: Generation) {
        if candidate.owns_shared {
            // Its own chain: dropping the old value frees the old chain.
            *self = candidate;
        } else {
            // Same chain: the old head frees only its program; the candidate
            // takes over the shared data.
            candidate.owns_shared = true;
            self.owns_shared = false;
            std::mem::swap(self, &mut candidate);
            // `candidate` is now the previous head: free its program before
            // the file versions only it read.
            drop(candidate);
            self.sweep_dead_files();
        }
    }

    /// Frees the regions of the file versions that no head reads any more:
    /// every file of the chain's full build stays (the shared processed-file
    /// data refers to them), and so does every file of this head. An edited
    /// file's replaced version, and the files of an abandoned candidate, go.
    /// Callable at any time on the building thread; nothing may hold a handle
    /// into a replaced file version (module invariant 1, which the commit
    /// protocol already requires of the replaced program).
    pub fn sweep_dead_files(&self) {
        if std::thread::current().id() != self.thread {
            return;
        }
        let dead = self
            .files
            .take_dead(self.program.source_files().iter().map(|file| file.addr()));
        drop(dead);
    }

    /// The file regions the chain holds, for tests of the lifecycle.
    #[must_use]
    pub fn file_region_count(&self) -> usize {
        self.files.len()
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        if std::thread::current().id() != self.thread || self.slot.outstanding() != 0 {
            // Module invariants 2 and 3: freeing here could free memory a live
            // reference or another thread's arena still uses. Leak instead,
            // and keep the checker slot alive forever.
            std::mem::forget(Arc::clone(&self.slot));
            std::mem::forget(Arc::clone(&self.regions));
            std::mem::forget(Arc::clone(&self.files));
            return;
        }
        let disposed = self.slot.dispose();
        debug_assert!(disposed, "no lease is outstanding (checked above)");
        let shared = shared_program_data(self.program);
        // SAFETY: invariant 1 (callers copied everything out), 2 (no checker
        // lease is outstanding, so no handle into this program exists) and 3
        // (this is the building thread). The program came from `new_program`
        // or `reuse_program`, and a head is dropped at most once.
        unsafe { free_program(self.program) };
        if self.owns_shared {
            // SAFETY: this head owns the chain's shared data, and every other
            // head of the chain was freed before it: `commit` makes the new
            // head the owner only after the old one gave it up, and a chain
            // has one live head. Nothing else refers to the data.
            unsafe { shared.free() };
        }
    }
}

/// The wasm32-wasip1 reactor's exports: the Go reactor's ABI
/// (`apps/solid-typefacts/reactor_wasip1.go`), byte for byte, over
/// [`crate::reactor::Abi`]. The host loads the module with
/// `wasi.initialize(instance)` and never calls a `main`.
///
/// The only `unsafe` here is the `unsafe(no_mangle)` attribute edition 2024
/// requires on an exported symbol; the bodies are safe. The state lives in a
/// thread-local because tsrs state is thread-local and wasm32-wasip1 runs
/// everything on its one thread.
#[cfg(all(feature = "reactor", target_arch = "wasm32"))]
mod reactor_exports {
    use crate::reactor::{Abi, open_project};
    use std::cell::RefCell;

    thread_local! {
        static ABI: RefCell<Abi> = RefCell::new(Abi::new(open_project));
    }

    fn with<R>(operation: impl FnOnce(&mut Abi) -> R) -> R {
        // A re-entrant call would be a second exchange on the same reactor;
        // the host never makes one (it cannot call back into a running
        // export), and a RefCell panic here aborts the module.
        ABI.with(|abi| operation(&mut abi.borrow_mut()))
    }

    // SAFETY: the export names are unique in the module; this is the only
    // place that defines them.
    #[unsafe(no_mangle)]
    pub extern "C" fn allocate_input(size: u32) -> u32 {
        with(|abi| abi.allocate_input(size))
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn output_pointer() -> u32 {
        with(|abi| abi.output_pointer())
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn output_length() -> u32 {
        with(|abi| abi.output_length())
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn typefacts_handshake() -> u32 {
        with(Abi::handshake)
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn typefacts_open() -> u32 {
        with(Abi::open)
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn typefacts_lifecycle(handle: u32) -> u32 {
        with(|abi| abi.lifecycle(handle))
    }

    // SAFETY: as above.
    #[unsafe(no_mangle)]
    pub extern "C" fn typefacts_close(handle: u32) -> u32 {
        with(|abi| abi.close(handle))
    }
}

/// `fd_filestat_set_times` is the one WASI import tsrs's file system pulls in
/// that the owned boundary does not cover (`owned-boundary.mjs`): wasi-libc
/// declares it as `__imported_wasi_snapshot_preview1_fd_filestat_set_times`,
/// and std's `File::set_times` reaches it from tsrs's `chtimes`. `chtimes`
/// serves build-status files of `tsc --build`; this producer never reaches it,
/// and a read-only owned mount refuses a write anyway. The module is linked
/// with `--wrap` on that symbol (packages/wasm/scripts/build-owned.mjs), so the
/// reference binds to the function below and the module declares no import the
/// boundary would refuse. It answers `ENOTSUP`, the honest answer for a file
/// system whose times cannot be set.
#[cfg(all(feature = "reactor", target_arch = "wasm32"))]
mod wasi_import_stubs {
    const ENOTSUP: i32 = 58;

    // SAFETY: the name is the `__wrap_` spelling the linker binds the
    // reference to; nothing else defines it, and the signature is preview1's
    // `(fd, atim, mtim, fst_flags) -> errno`.
    #[allow(dead_code)]
    #[unsafe(no_mangle)]
    extern "C" fn __wrap___imported_wasi_snapshot_preview1_fd_filestat_set_times(
        _fd: i32,
        _atim: i64,
        _mtim: i64,
        _flags: i32,
    ) -> i32 {
        ENOTSUP
    }
}
