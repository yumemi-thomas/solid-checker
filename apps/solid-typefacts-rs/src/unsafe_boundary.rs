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
//!    data, `SharedProgramData::free`; finally the chain's [`ProgramRegions`]
//!    when the last `Arc` drops, which is after everything that points into
//!    them is gone.
//! 5. **Shared data is freed exactly once**, by the head that owns it. A
//!    head made by [`Generation::prepare_update`] from a reusable previous
//!    head does not own it until [`Generation::commit`] transfers ownership,
//!    so an update that is abandoned frees only its own program.
//! 6. Diagnostics point into checker regions; callers copy `code/pos/end`
//!    before the checker is released.
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
    regions: Arc<ProgramRegions>,
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
    pub fn build(config: P<ParsedCommandLine>, host: Arc<dyn CompilerHost>) -> Generation {
        let regions = Arc::new(ProgramRegions::default());
        let (factory, cell) = slot_factory();
        let program = new_program(options_for(config, host, &regions, &factory));
        program.bind_source_files();
        Generation {
            program,
            slot: take_slot(&cell),
            regions,
            owns_shared: true,
            updates: 0,
            thread: std::thread::current().id(),
            kind: UpdateKind::Rebuilt,
        }
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
    pub fn with_checker<R>(&self, f: impl FnOnce(&mut Checker) -> R) -> R {
        let mut handle = self.program.get_type_checker(&Context::default());
        f(&mut handle)
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
            Arc::clone(&host),
            Some(Arc::clone(&factory)),
            None,
        );
        if let Some(program) = reused {
            program.bind_source_files();
            return Generation {
                program,
                slot: take_slot(&cell),
                regions: Arc::clone(&self.regions),
                owns_shared: false,
                updates: self.updates + 1,
                thread: self.thread,
                kind: UpdateKind::Reused,
            };
        }
        // A fresh chain from the same parsed configuration, in regions of its
        // own, so replacing this head frees the whole old chain.
        let regions = Arc::new(ProgramRegions::default());
        let program = new_program(options_for(
            self.program.command_line(),
            host,
            &regions,
            &factory,
        ));
        program.bind_source_files();
        Generation {
            program,
            slot: take_slot(&cell),
            regions,
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
        }
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
