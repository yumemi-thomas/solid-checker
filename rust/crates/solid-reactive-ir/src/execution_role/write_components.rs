//! ADR 0546: the write walk's caller fold, computed once per function of the
//! region its callers reach instead of once per path.
//!
//! [`super::write_roles::caller_loop`] folds the entries of a function (its
//! call sites, each placed by its own walk) and cuts a call written in a
//! function the walk is already deriving. Followed literally
//! ([`super::write_roles`]'s `path_fold`), that visits every simple path
//! through the callers: factorial in functions that all call each other.
//! The answer is path-dependent in general (a function whose callers are all
//! on the path folds to nothing), but two facts make it computable once per
//! function of the region where every edge is plain:
//!
//! - A rejecting entry ends every fold above it unchanged, and a revisit of
//!   a function a path already left reaches no function and no entry its
//!   first visit did not: the first rejection the paths meet is the one a
//!   depth-first search with one visited set meets ([`Region::first_rejection`]).
//! - Without a rejection, `Unknown` is absorbing: a fold is `Unknown` as soon
//!   as one entry is, so a fold stops at its first `Unknown`
//!   ([`Region::eval`]); a region where no fold can be `Unknown` folds to the
//!   first role its first entries lead to ([`Region::chain`]).
//!
//! Every entry is placed once, in the walk's state at the region's root plus
//! the function it is an entry of, and every query the placement makes of
//! the walk's `visiting` set is recorded ([`visit_contains`],
//! [`visit_insert`]). The region is used only when no placement asked about
//! a function of the region other than its own and its caller's (absent on
//! every path that places it), so each placement is the one every path would
//! compute; otherwise, and for an edge whose loop does not fold plainly, the
//! walk follows the paths.
//!
//! With the role log on, the walk follows the paths: the log records the arms
//! every path passes, which this fold does not visit.

use super::write_roles::{
    CallerFold, caller_loop, entry_census_complete, event_only_entries, plain_call_outside_jsx,
    write_allowed_spans, write_role_fallback, write_role_walk,
};
use super::*;

/// Set by [`follow_every_path`].
static EVERY_PATH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// ADR 0546, for tests: with `on`, every walk this ADR folds follows its
/// paths as written instead (the write walk's callers, the helper climb,
/// the site-reach derivation, the ordering proof), in this process, until
/// it is called again. The answers are the same either way; a test builds
/// a program both ways and compares them.
pub fn follow_every_path(on: bool) {
    EVERY_PATH.store(on, std::sync::atomic::Ordering::Release);
}

/// Whether the walks follow every path: [`follow_every_path`], or the role
/// log on (it records the arms every path passes), or under a probe or a
/// proof's deferred check (their thread state is not what a fold reads).
pub(crate) fn every_path() -> bool {
    EVERY_PATH.load(std::sync::atomic::Ordering::Acquire)
        || crate::role_log::enabled()
        || crate::indexes::probing()
        || crate::base::memo::derived::is_deferring()
}

/// Whether [`follow_every_path`] is on: what the walks that log no arms
/// (the site-reach derivation, the ordering proof) follow.
pub(crate) fn every_path_asked() -> bool {
    EVERY_PATH.load(std::sync::atomic::Ordering::Acquire)
}

/// A function's or a marker's entry in the walk's `visiting` set.
type Key = (String, Span);

/// The work a region fold may do before the walk follows the paths instead.
const STEP_BUDGET: usize = 1 << 20;

thread_local! {
    /// The `visiting` queries made while a placement is recorded.
    static VISITS: std::cell::RefCell<VisitLog> = std::cell::RefCell::default();
}

#[derive(Default)]
struct VisitLog {
    /// The recordings in progress on this thread.
    recording: usize,
    /// Each query since the outermost recording began: the queried set's
    /// address and the key.
    queries: Vec<(usize, Key)>,
}

fn note(visiting: &HashSet<Key>, key: &Key) {
    VISITS.with(|log| {
        let mut log = log.borrow_mut();
        if log.recording > 0 {
            log.queries.push((address(visiting), key.clone()));
        }
    });
}

/// `visiting.contains(key)`, recorded for a region fold.
pub(super) fn visit_contains(visiting: &HashSet<Key>, key: &Key) -> bool {
    note(visiting, key);
    visiting.contains(key)
}

/// `visiting.insert(key)`, recorded for a region fold.
pub(super) fn visit_insert(visiting: &mut HashSet<Key>, key: Key) -> bool {
    note(visiting, &key);
    visiting.insert(key)
}

/// The address [`note`] knows a `visiting` set by.
fn address(visiting: &HashSet<Key>) -> usize {
    std::ptr::from_ref(visiting) as usize
}

/// Runs `f` with the queries of the `visiting` set at `address` recorded;
/// returns its answer and the keys it asked about.
fn recorded<R>(address: usize, f: impl FnOnce() -> R) -> (R, HashSet<Key>) {
    struct Recording;
    impl Drop for Recording {
        fn drop(&mut self) {
            VISITS.with(|log| {
                let mut log = log.borrow_mut();
                log.recording -= 1;
                if log.recording == 0 {
                    log.queries.clear();
                }
            });
        }
    }
    let start = VISITS.with(|log| {
        let mut log = log.borrow_mut();
        log.recording += 1;
        log.queries.len()
    });
    let recording = Recording;
    let value = f();
    let keys = VISITS.with(|log| {
        log.borrow().queries[start..]
            .iter()
            .filter(|(set, _)| *set == address)
            .map(|(_, key)| key.clone())
            .collect()
    });
    drop(recording);
    (value, keys)
}

/// One call site of a region function, as the fold reads it.
#[derive(Clone, Copy, Debug)]
enum Entry {
    /// A call in a function the walk was deriving before the region began
    /// (or the function itself): plain, so its role is that function's.
    Skip,
    /// The same, not plain: the entry has a role of its own nobody derives.
    Unplaced,
    /// A call whose walk answers this, on every path.
    Value(ExecutionRole),
    /// A plain call in region function `to`, whose loop folds plainly: the
    /// entry's role is that fold; on a path through `to` it is cut, as a
    /// `Skip` when `plain` and an `Unplaced` otherwise.
    Edge { to: usize, plain: bool },
}

struct Node<'a> {
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
    key: Key,
    entries: Vec<Entry>,
    census: Option<bool>,
}

/// A placed call site: the function it is an entry of, the function it is
/// written in, and the keys its placement asked `visiting` about.
struct Placed {
    node: usize,
    caller: Option<Key>,
    value: bool,
    queried: HashSet<Key>,
}

struct Region<'a, 'y: 'a, 'v, 'l> {
    nodes: Vec<Node<'a>>,
    index: HashMap<Key, usize>,
    placed: Vec<Placed>,
    visiting: &'v mut HashSet<Key>,
    entities: &'l EntitySymbols,
    symbol_names: &'l HashMap<SymbolId, SymbolId>,
    lookup: &'l SemanticLookup<'y>,
    root_body: RootBodyGuard,
    steps: usize,
}

/// ADR 0546: [`caller_loop`]'s fold of `function`'s entries, computed over
/// the region of functions its plain callers reach; `None` where the region
/// does not decide it exactly (the walk then follows the paths).
#[allow(clippy::too_many_arguments)]
pub(super) fn region_fold(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    imperative: Option<ExecutionRole>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<Key>,
) -> Option<CallerFold> {
    if every_path() {
        return None;
    }
    let mut region = Region {
        nodes: Vec::new(),
        index: HashMap::default(),
        placed: Vec::new(),
        visiting,
        entities,
        symbol_names,
        lookup,
        root_body,
        steps: 0,
    };
    region.intern(file, function);
    let folded = region.fold(imperative);
    lookup.count_write_walk_steps(region.steps as u64);
    folded
}

impl<'a, 'y: 'a> Region<'a, 'y, '_, '_> {
    fn intern(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> usize {
        let key = (file.path.to_string(), function.span);
        if let Some(&node) = self.index.get(&key) {
            return node;
        }
        let node = self.nodes.len();
        self.index.insert(key.clone(), node);
        self.nodes.push(Node {
            file,
            function,
            key,
            entries: Vec::new(),
            census: None,
        });
        node
    }

    fn step(&mut self) -> Option<()> {
        self.steps += 1;
        (self.steps <= STEP_BUDGET).then_some(())
    }

    fn fold(&mut self, mut imperative: Option<ExecutionRole>) -> Option<CallerFold> {
        let (rejection, visited) = self.first_rejection()?;
        self.independent(&visited)?;
        if let Some(role) = rejection {
            return Some(CallerFold::Rejected(role));
        }
        // No entry anywhere rejects: the root's fold is unplaced at its first
        // `Unknown` entry, and otherwise takes its first role.
        let mut on_path = vec![false; self.nodes.len()];
        on_path[0] = true;
        let mut unplaced = false;
        for index in 0..self.nodes[0].entries.len() {
            self.step()?;
            let role = match self.nodes[0].entries[index] {
                Entry::Skip => continue,
                Entry::Unplaced => ExecutionRole::Unknown,
                Entry::Value(role) => role,
                Entry::Edge { to, .. } => self.eval(to, &mut on_path)?,
            };
            if role == ExecutionRole::Unknown {
                unplaced = true;
                break;
            }
            imperative.get_or_insert(role);
        }
        Some(CallerFold::Entries {
            imperative,
            unplaced,
        })
    }

    /// Places the entries of every function the root's plain callers reach,
    /// in depth-first order with one visited set, until an entry rejects the
    /// write: the rejection every path meets first. Answers it (none when no
    /// entry rejects) and the functions placed.
    fn first_rejection(&mut self) -> Option<(Option<ExecutionRole>, Vec<bool>)> {
        self.place(0)?;
        let mut visited = vec![true];
        let mut stack = vec![(0usize, 0usize)];
        while let Some(&(node, index)) = stack.last() {
            let Some(&entry) = self.nodes[node].entries.get(index) else {
                stack.pop();
                continue;
            };
            stack.last_mut()?.1 += 1;
            self.step()?;
            match entry {
                Entry::Value(role) if role.reports_disallowed_write() => {
                    return Some((Some(role), visited));
                }
                Entry::Edge { to, .. } => {
                    visited.resize(self.nodes.len(), false);
                    if !visited[to] {
                        visited[to] = true;
                        self.place(to)?;
                        stack.push((to, 0));
                    }
                }
                _ => {}
            }
        }
        visited.resize(self.nodes.len(), false);
        Some((None, visited))
    }

    /// Places each call site of region function `node` as the walk does
    /// with `node` in progress ([`caller_loop`]), in the walk's state at the
    /// root plus `node`.
    fn place(&mut self, node: usize) -> Option<()> {
        let (file, function, key) = {
            let node = &self.nodes[node];
            (node.file, node.function, node.key.clone())
        };
        let lookup = self.lookup;
        // The root is in progress already; any other region function is not
        // (a query found it absent before it joined).
        let entered = node != 0 && visit_insert(self.visiting, key.clone());
        let placed = self.place_sites(node, file, function, lookup);
        if entered {
            self.visiting.remove(&key);
        }
        let entries = placed?;
        self.nodes[node].entries = entries;
        Some(())
    }

    fn place_sites(
        &mut self,
        node: usize,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        lookup: &SemanticLookup<'y>,
    ) -> Option<Vec<Entry>> {
        let mut entries = Vec::new();
        for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
            self.step()?;
            let caller = crate::owners::containing_body_function(&caller_file.ast, callee);
            let caller_key = caller.map(|caller| (caller_file.path.to_string(), caller.span));
            if let (Some(caller), Some(caller_key)) = (caller, &caller_key)
                && visit_contains(self.visiting, caller_key)
            {
                entries.push(if plain_call_outside_jsx(caller_file, caller, callee) {
                    Entry::Skip
                } else {
                    Entry::Unplaced
                });
                continue;
            }
            let caller_allowed = write_allowed_spans(
                caller_file,
                &allowed_callback_spans(caller_file, lookup),
                lookup,
            );
            let (entities, symbol_names, root_body) =
                (self.entities, self.symbol_names, self.root_body);
            let at = address(self.visiting);
            let visiting = &mut *self.visiting;
            let mut decided = false;
            let mut plan = None;
            let (role, mut queried) = recorded(at, || {
                write_role_walk(
                    caller_file,
                    callee,
                    &caller_allowed,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    visiting,
                    &mut decided,
                    Some(&mut plan),
                )
            });
            let role = match plan {
                Some(plan) => {
                    if let Some(caller) = caller
                        && plan.function.span == caller.span
                        && plan.plain()
                    {
                        // The entry's role is the caller's fold; an `Unknown`
                        // fold must stay `Unknown` past the walk's fallback.
                        let unplaced = write_role_fallback(
                            caller_file,
                            callee,
                            ExecutionRole::Unknown,
                            decided,
                            root_body,
                            lookup,
                        );
                        if unplaced != ExecutionRole::Unknown {
                            return None;
                        }
                        let to = self.intern(caller_file, caller);
                        entries.push(Entry::Edge {
                            to,
                            plain: plain_call_outside_jsx(caller_file, caller, callee),
                        });
                        self.placed.push(Placed {
                            node,
                            caller: caller_key,
                            value: false,
                            queried,
                        });
                        continue;
                    }
                    // A loop this region does not fold: its answer, computed
                    // here, is the entry's when it asked about no region
                    // function.
                    let visiting = &mut *self.visiting;
                    let (role, more) = recorded(at, || {
                        caller_loop(
                            caller_file,
                            plan,
                            entities,
                            symbol_names,
                            lookup,
                            root_body,
                            visiting,
                        )
                    });
                    queried.extend(more);
                    role
                }
                None => role,
            };
            let role = write_role_fallback(caller_file, callee, role, decided, root_body, lookup);
            entries.push(Entry::Value(role));
            self.placed.push(Placed {
                node,
                caller: caller_key,
                value: true,
                queried,
            });
        }
        Some(entries)
    }

    /// Whether every placement is the one each path computes: none asked
    /// about a region function other than the one it is an entry of (in
    /// progress on every path) and the one it is written in (absent on every
    /// path that places it), and no answered entry is written in a region
    /// function (a path through it would cut the entry).
    fn independent(&self, visited: &[bool]) -> Option<()> {
        let region = |key: &Key| {
            self.index
                .get(key)
                .copied()
                .filter(|&node| node != 0 && visited.get(node).copied().unwrap_or(false))
        };
        for placed in &self.placed {
            if placed.value && placed.caller.as_ref().and_then(region).is_some() {
                return None;
            }
            for key in &placed.queried {
                if let Some(node) = region(key)
                    && node != placed.node
                    && placed.caller.as_ref() != Some(key)
                {
                    return None;
                }
            }
        }
        Some(())
    }

    fn census(&mut self, node: usize) -> bool {
        if let Some(census) = self.nodes[node].census {
            return census;
        }
        let (file, function) = (self.nodes[node].file, self.nodes[node].function);
        let census = entry_census_complete(file, function, self.lookup)
            || event_only_entries(file, function, self.lookup);
        self.nodes[node].census = Some(census);
        census
    }

    /// The fold of region function `node` entered with the functions in
    /// `on_path` in progress, where no entry rejects: `Unknown` at its first
    /// unplaced entry; otherwise its first role, under a complete census.
    fn eval(&mut self, node: usize, on_path: &mut Vec<bool>) -> Option<ExecutionRole> {
        if let Some(role) = self.chain(node, on_path)? {
            return Some(role);
        }
        on_path[node] = true;
        let folded = self.eval_entries(node, on_path);
        on_path[node] = false;
        folded
    }

    fn eval_entries(&mut self, node: usize, on_path: &mut Vec<bool>) -> Option<ExecutionRole> {
        let mut imperative = None;
        for index in 0..self.nodes[node].entries.len() {
            self.step()?;
            let role = match self.nodes[node].entries[index] {
                Entry::Skip => continue,
                Entry::Unplaced => ExecutionRole::Unknown,
                Entry::Value(role) => role,
                Entry::Edge { to, plain } if on_path[to] => {
                    if plain {
                        continue;
                    }
                    ExecutionRole::Unknown
                }
                Entry::Edge { to, .. } => self.eval(to, on_path)?,
            };
            if role == ExecutionRole::Unknown {
                return Some(ExecutionRole::Unknown);
            }
            imperative.get_or_insert(role);
        }
        Some(match imperative {
            Some(role) if self.census(node) => role,
            _ => ExecutionRole::Unknown,
        })
    }

    /// Where no fold reachable from `node` (off `on_path`) can be `Unknown`
    /// (every such function has an entry with a role, a complete census, and
    /// no unplaced entry or edge), the fold is the first role its first
    /// uncut entries lead to; `Some(None)` where that is not shown.
    fn chain(&mut self, node: usize, on_path: &mut [bool]) -> Option<Option<ExecutionRole>> {
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = vec![node];
        seen[node] = true;
        while let Some(at) = queue.pop() {
            self.step()?;
            let mut role = false;
            for index in 0..self.nodes[at].entries.len() {
                match self.nodes[at].entries[index] {
                    Entry::Skip => {}
                    Entry::Unplaced | Entry::Value(ExecutionRole::Unknown) => return Some(None),
                    Entry::Value(_) => role = true,
                    Entry::Edge { plain: false, .. } => return Some(None),
                    Entry::Edge { to, .. } => {
                        if !on_path[to] && !seen[to] {
                            seen[to] = true;
                            queue.push(to);
                        }
                    }
                }
            }
            if !role || !self.census(at) {
                return Some(None);
            }
        }
        let mut path = Vec::new();
        let mut at = node;
        let role = 'chain: loop {
            on_path[at] = true;
            path.push(at);
            for index in 0..self.nodes[at].entries.len() {
                self.step()?;
                match self.nodes[at].entries[index] {
                    Entry::Value(role) => break 'chain role,
                    Entry::Edge { to, .. } if !on_path[to] => {
                        at = to;
                        continue 'chain;
                    }
                    _ => {}
                }
            }
            // Every function here has an entry with a role, so the scan
            // never ends without one; if it did, the paths decide.
            return None;
        };
        for at in path {
            on_path[at] = false;
        }
        Some(Some(role))
    }
}
