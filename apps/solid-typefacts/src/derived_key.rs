//! Checker-independent keys for derived symbols (instant-edits P2, ADR
//! 03XX-p6).
//!
//! A derived symbol (an instantiation, a transient clone, a mapped or a
//! synthetic property) is keyed by what it is derived from and by the exact
//! type context the checker derived it in, never by erasure: two symbols one
//! checker keeps apart by type never share a key (`Foo<number>.x` and
//! `Foo<string>.x` differ), and two checkers deriving the same symbol give it
//! the same key, whatever each checked first.
//!
//! * Instantiated (`target` and `mapper`): when instantiation cannot affect
//!   the root (its type, and a set accessor's write type, could contain no
//!   type variable), the checker itself returns the root once those types are
//!   resolved, so the key is the root's; otherwise `inst(root, mapper)`.
//! * Clone (`target`, no `mapper`): `clone(root, type of the clone)`.
//! * Mapped property: `mapped(name, containing mapped type)`.
//! * Synthetic union/intersection property outside the `Set` key:
//!   `synth(name, containing type)`.
//! * Tuple members by their tuple target; tsrs's shared tuple element type
//!   parameters by index.
//! * Identities no derivation link names ([`Keyer::special`]): checker
//!   singletons by field, an unresolved entity name by its path, a class's
//!   binder `prototype` by its class, a module re-typed for a namespace import
//!   by module and import.
//! * A binder symbol `binder_key` refuses: [`binder_fallback`].
//!
//! Types are keyed structurally ([`Keyer::type_key`]): intrinsics by which
//! checker singleton they are, literals by value, named and anonymous types by
//! their declaration's key, references by target and type arguments,
//! instantiated anonymous types by target and mapper, unions and
//! intersections as key sets, recursion by back-reference. Anything else (an
//! index-signature symbol, a binding-pattern property, a type kind not keyed
//! here, a depth past [`MAX_DEPTH`], recursion past [`CYCLIC_BUDGET`]) has no
//! key, and the symbol then has no id (`Identities::id_for`): an absent fact.

use crate::binder_key::{Key, binder_key, set_key};
use crate::durable::{DurableRef, normalized_name};
use crate::gocompat::clean;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tsrs_ast::{CheckFlags, Node, Symbol, SymbolFlags, get_source_file_of_node};
use tsrs_checker::mapper::{TypeMapper, TypeMapperData};
use tsrs_checker::types::{LiteralValue, Type, TypeData};
use tsrs_compiler::Checker;
use tsrs_core::P;

pub(crate) type Digest32 = [u8; 32];

/// Opt-in (measurement): a file the keyer appends one line per decision to.
pub const ENV_STATS: &str = "SOLID_TYPEFACTS_DERIVED_KEY_STATS";

/// Tuple targets' type parameters and member symbols, by address, to the
/// target and their role. A tuple target's element properties and `length`
/// are transient symbols with no link back to it, and its type parameters
/// have no symbol: this index is how they are keyed by their target.
#[derive(Default)]
pub(crate) struct TupleIndex {
    targets: usize,
    roles: HashMap<usize, (P<Type>, TupleRole)>,
}

#[derive(Clone)]
enum TupleRole {
    Parameter(usize),
    Member,
}

impl TupleIndex {
    fn lookup(&mut self, checker: &Checker, address: usize) -> Option<(P<Type>, TupleRole)> {
        if let Some(found) = self.roles.get(&address) {
            return Some(found.clone());
        }
        let count = checker.tuple_types.values().count();
        if count == self.targets {
            return None;
        }
        self.targets = count;
        self.roles.clear();
        for target in checker.tuple_types.values() {
            let TypeData::Tuple(tuple) = target.data() else {
                continue;
            };
            for (index, parameter) in tuple
                .interface_type
                .all_type_parameters
                .get()
                .iter()
                .enumerate()
            {
                self.roles
                    .insert(parameter.addr(), (target, TupleRole::Parameter(index)));
            }
            if let Some(members) = tuple.interface_type.declared_members.get() {
                for member in members.values() {
                    self.roles
                        .insert(member.addr(), (target, TupleRole::Member));
                }
            }
        }
        self.roles.get(&address).cloned()
    }
}

const MAX_DEPTH: u32 = 40;

/// Cyclic subtrees one derived symbol's key may visit. A recursive type is
/// keyed by its unfolding, which repeats each cycle once per path through
/// it; past this the symbol has no key (no id: fail closed).
const CYCLIC_BUDGET: u32 = 20_000;

/// How a derived symbol was keyed, or why it was not (measurement).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Outcome {
    Root,
    Instantiated,
    Clone,
    Mapped,
    Synthetic,
    NoKind,
    NoRoot,
    NoType,
    Tuple,
    Singleton,
    Unresolved,
    Prototype,
    ModuleType,
    BinderFallback,
}

pub(crate) struct Keyer<'a, F: Fn(&DurableRef) -> bool> {
    pub checker: &'a mut Checker,
    pub exported: F,
    pub memo: &'a mut HashMap<usize, (Option<Digest32>, u32)>,
    pub tuples: &'a mut TupleIndex,
    /// Types being keyed, outermost first: a revisit is a back-reference.
    stack: Vec<usize>,
    /// Whether the subtree being keyed holds a back-reference.
    cyclic: bool,
    /// Whether the subtree being keyed failed by depth or by the cyclic
    /// budget (path-dependent: never memoized).
    path_failed: bool,
    /// The deepest `depth` the subtree being keyed reached.
    reach: u32,
    /// Back-references and cyclic subtrees keyed for this symbol, against
    /// [`CYCLIC_BUDGET`].
    pub cyclic_work: u32,
    /// The first unkeyable part, for measurement.
    pub failure: Option<String>,
    pub dump: Option<String>,
}

struct Enc(Vec<u8>);

impl Enc {
    fn new(tag: &str) -> Self {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(tag.as_bytes());
        bytes.push(0);
        Self(bytes)
    }
    fn str(&mut self, value: &str) -> &mut Self {
        self.0.extend_from_slice(value.len().to_string().as_bytes());
        self.0.push(b':');
        self.0.extend_from_slice(value.as_bytes());
        self.0.push(0);
        self
    }
    fn digest(&mut self, value: &Digest32) -> &mut Self {
        self.0.extend_from_slice(value);
        self
    }
    fn u64(&mut self, value: u64) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }
    fn finish(&self) -> Digest32 {
        Sha256::digest(&self.0).into()
    }
}

/// A binder symbol `binder_key` refuses (it does not own every declaration:
/// a parameter property's parameter beside its property, a symbol beside its
/// local half): it is one program-arena object in every checker, so a key over
/// its minimum declaration, flags and name, in a domain of its own, is a
/// function of the program and separates it from its sibling.
pub(crate) fn binder_fallback(symbol: P<Symbol>) -> Option<Key> {
    if symbol.flags().intersects(SymbolFlags::Transient) {
        return None;
    }
    let reference = crate::durable::minimum_declaration_ref(symbol)?;
    let mut context = Enc::new("binder-fallback");
    context
        .str(&reference.path)
        .u64(u64::from(reference.start_byte))
        .u64(u64::from(reference.end_byte))
        .u64(u64::from(symbol.flags().bits()));
    Some(Key::Derived {
        tag: "binder",
        name: reference.name,
        root: String::new(),
        context: context.finish(),
    })
}

fn node_location(node: P<Node>) -> Option<(String, u32, u32)> {
    let file = get_source_file_of_node(node)?;
    Some((
        clean(file.file_name()),
        node.pos().source_offset()?,
        node.end().source_offset()?,
    ))
}

impl<'a, F: Fn(&DurableRef) -> bool> Keyer<'a, F> {
    fn need<T>(&mut self, value: Option<T>, why: &'static str) -> Option<T> {
        if value.is_none() {
            self.failure.get_or_insert_with(|| why.to_owned());
        }
        value
    }

    pub(crate) fn new(
        checker: &'a mut Checker,
        exported: F,
        memo: &'a mut HashMap<usize, (Option<Digest32>, u32)>,
        tuples: &'a mut TupleIndex,
    ) -> Self {
        Self {
            checker,
            exported,
            memo,
            tuples,
            stack: Vec::new(),
            cyclic: false,
            path_failed: false,
            reach: 0,
            cyclic_work: 0,
            failure: None,
            dump: None,
        }
    }

    /// The id string of any symbol: binder, `Set`, or derived.
    fn symbol_id(&mut self, symbol: P<Symbol>, depth: u32) -> Option<String> {
        if let Some((key, _)) = binder_key(self.checker, symbol, &self.exported) {
            return Some(key.id());
        }
        if let Some(key) = set_key(self.checker, symbol, &self.exported) {
            return Some(key.id());
        }
        if let Some(key) = binder_fallback(symbol) {
            return Some(key.id());
        }
        self.derived(symbol, depth).0.map(|key| key.id())
    }

    /// The key of a derived symbol, and how it was (or was not) keyed.
    pub(crate) fn derived(&mut self, symbol: P<Symbol>, depth: u32) -> (Option<Key>, Outcome) {
        self.derived_inner(symbol, depth)
    }

    fn derived_inner(&mut self, symbol: P<Symbol>, depth: u32) -> (Option<Key>, Outcome) {
        self.reach = self.reach.max(depth);
        if depth > MAX_DEPTH {
            self.path_failed = true;
            self.failure.get_or_insert_with(|| "depth".into());
            return (None, Outcome::NoType);
        }
        let name = normalized_name(symbol.name()).into_owned();
        if let Some(found) = self.special(symbol, depth) {
            return found;
        }
        let links = self
            .checker
            .value_symbol_links
            .try_get_if_id_assigned(symbol);
        let (target, mapper, containing) = match links {
            Some(links) => (links.target(), links.mapper(), links.containing_type()),
            None => (None, None, None),
        };
        if let Some(root) = target {
            let Some(root_id) = self.symbol_id(root, depth + 1) else {
                return (None, Outcome::NoRoot);
            };
            if let Some(mapper) = mapper {
                // `is_symbol_unaffected_by_instantiation`, decided on the
                // resolved root type rather than on whatever the checker had
                // resolved when it instantiated. (Its `maps_this_only` arm is
                // structural, so no checker derives a symbol through it.) A set
                // accessor's write type decides too and is not reachable here:
                // fail closed.
                let root_type = self.checker.get_type_of_symbol_exported(root);
                let mut unaffected = !self
                    .checker
                    .could_contain_type_variables_exported(root_type);
                // A set accessor answers for its instantiation only when its
                // write type, cached on its links once resolved, could not
                // contain type variables either.
                if unaffected && root.flags().intersects(SymbolFlags::SetAccessor) {
                    unaffected = self
                        .checker
                        .get_write_type_of_symbol_exported(root)
                        .is_some_and(|write| {
                            !self.checker.could_contain_type_variables_exported(write)
                        });
                }
                if unaffected {
                    return (
                        Some(Key::Derived {
                            tag: "root",
                            name,
                            root: root_id,
                            context: [0; 32],
                        }),
                        Outcome::Root,
                    );
                }
                let Some(context) = self.mapper_key(mapper, depth + 1) else {
                    return (None, Outcome::NoType);
                };
                return (
                    Some(Key::Derived {
                        tag: "inst",
                        name,
                        root: root_id,
                        context,
                    }),
                    Outcome::Instantiated,
                );
            }
            let own = self.checker.get_type_of_symbol_exported(symbol);
            let Some(context) = self.type_key(own, depth + 1) else {
                return (None, Outcome::NoType);
            };
            return (
                Some(Key::Derived {
                    tag: "clone",
                    name,
                    root: root_id,
                    context,
                }),
                Outcome::Clone,
            );
        }
        if let Some(containing) = containing {
            let Some(context) = self.type_key(containing, depth + 1) else {
                return (None, Outcome::NoType);
            };
            let tag = if symbol.check_flags().intersects(CheckFlags::Mapped) {
                "mapped"
            } else {
                "synth"
            };
            let outcome = if tag == "mapped" {
                Outcome::Mapped
            } else {
                Outcome::Synthetic
            };
            return (
                Some(Key::Derived {
                    tag,
                    name,
                    root: String::new(),
                    context,
                }),
                outcome,
            );
        }
        if let Some((target, TupleRole::Member)) = self.tuples.lookup(self.checker, symbol.addr()) {
            let Some(context) = self.type_key(target, depth + 1) else {
                return (None, Outcome::NoType);
            };
            return (
                Some(Key::Derived {
                    tag: "tuple-member",
                    name,
                    root: String::new(),
                    context,
                }),
                Outcome::Tuple,
            );
        }
        self.failure.get_or_insert_with(|| {
            format!(
                "nokind:flags={:?}:check={:?}:decls={}:name={}",
                symbol.flags(),
                symbol.check_flags(),
                symbol.declarations().len(),
                symbol.name()
            )
        });
        (None, Outcome::NoKind)
    }

    /// Derived symbols with an identity of their own that no derivation link
    /// names: checker singletons, unresolved entity names, a class's
    /// `prototype`, a module re-typed for a namespace import. `None`: not
    /// one of them. An index-signature symbol has no exact key and none here.
    fn special(&mut self, symbol: P<Symbol>, depth: u32) -> Option<(Option<Key>, Outcome)> {
        let keyed =
            |tag: &'static str, name: String, root: String, context: Digest32| Key::Derived {
                tag,
                name,
                root,
                context,
            };
        if let Some(which) = singleton_symbol(self.checker, symbol) {
            return Some((
                Some(keyed("singleton", which.to_owned(), String::new(), [0; 32])),
                Outcome::Singleton,
            ));
        }
        let check = symbol.check_flags();
        if check.intersects(CheckFlags::Unresolved)
            && symbol.flags().intersects(SymbolFlags::TypeAlias)
        {
            // `get_unresolved_symbol_for_entity_name`: one symbol per checker
            // per entity-name path (`get_symbol_path`), so the path is exact.
            let mut parts = vec![symbol.name()];
            let mut parent = symbol.parent();
            while let Some(current) = parent {
                if parts.len() > 64 {
                    self.failure
                        .get_or_insert_with(|| "unresolved:depth".into());
                    return Some((None, Outcome::NoKind));
                }
                parts.push(current.name());
                parent = current.parent();
            }
            parts.reverse();
            return Some((
                Some(keyed("unresolved", parts.join("."), String::new(), [0; 32])),
                Outcome::Unresolved,
            ));
        }
        if check.intersects(CheckFlags::IndexSymbol) {
            // `get_applicable_index_symbol` caches one symbol per index info,
            // and an index info has no link to the type it belongs to: two
            // instantiations of one signature (`A<string>`, `A<boolean>`) can
            // share declarations and value type and still be two symbols.
            self.failure.get_or_insert_with(|| "index-symbol".into());
            return Some((None, Outcome::NoKind));
        }
        if symbol.flags().intersects(SymbolFlags::Prototype)
            && !symbol.flags().intersects(SymbolFlags::Transient)
            && symbol.declarations().is_empty()
        {
            // The binder makes one `prototype` per class symbol.
            let Some(parent) = symbol.parent() else {
                self.failure
                    .get_or_insert_with(|| "prototype:no-parent".into());
                return Some((None, Outcome::NoKind));
            };
            let Some(root) = self.symbol_id(parent, depth + 1) else {
                return Some((None, Outcome::NoRoot));
            };
            return Some((
                Some(keyed(
                    "prototype",
                    normalized_name(symbol.name()).into_owned(),
                    root,
                    [0; 32],
                )),
                Outcome::Prototype,
            ));
        }
        if let Some(links) = self.checker.export_type_links.try_get(symbol)
            && let Some(target) = links.target.get()
        {
            // `clone_type_as_module_type`: the module symbol re-typed for one
            // namespace import or import call, a function of both.
            let Some(import) = links.originating_import.get().and_then(node_location) else {
                self.failure
                    .get_or_insert_with(|| "module-type:no-import".into());
                return Some((None, Outcome::NoKind));
            };
            let Some(root) = self.symbol_id(target, depth + 1) else {
                return Some((None, Outcome::NoRoot));
            };
            let context = Enc::new("module-type")
                .str(&import.0)
                .u64(u64::from(import.1))
                .u64(u64::from(import.2))
                .finish();
            return Some((
                Some(keyed(
                    "module-type",
                    normalized_name(symbol.name()).into_owned(),
                    root,
                    context,
                )),
                Outcome::ModuleType,
            ));
        }
        None
    }

    fn mapper_key(&mut self, mapper: P<TypeMapper>, depth: u32) -> Option<Digest32> {
        let mut sources = Vec::new();
        let mut functions = Vec::new();
        let mut inferred = Vec::new();
        if collect_sources(
            self.checker,
            mapper,
            &mut sources,
            &mut functions,
            &mut inferred,
            0,
        )
        .is_none()
        {
            self.failure.get_or_insert_with(|| "mapper".into());
            return None;
        }
        let mut seen = HashSet::new();
        let mut pairs = Vec::new();
        for (source, image) in inferred {
            if seen.insert(source.addr()) {
                pairs.push((
                    self.type_key(source, depth + 1)?,
                    self.type_key(image, depth + 1)?,
                ));
            }
        }
        for source in sources {
            if !seen.insert(source.addr()) {
                continue;
            }
            let image = mapper.map(self.checker, source);
            pairs.push((
                self.type_key(source, depth + 1)?,
                self.type_key(image, depth + 1)?,
            ));
        }
        if std::env::var_os("SOLID_TYPEFACTS_DK_DUMP").is_some() {
            let mut dump = Vec::new();
            let mut all = Vec::new();
            let mut functions2 = Vec::new();
            let mut inferred2 = Vec::new();
            let _ = collect_sources(
                self.checker,
                mapper,
                &mut all,
                &mut functions2,
                &mut inferred2,
                0,
            );
            for source in all {
                let image = mapper.map(self.checker, source);
                let key = self
                    .type_key(image, depth + 1)
                    .map(|k| format!("{:02x}{:02x}", k[0], k[1]))
                    .unwrap_or_default();
                let text = self.checker.type_to_string_exported(image);
                let source_key = self
                    .type_key(source, depth + 1)
                    .map(|k| format!("{:02x}{:02x}", k[0], k[1]))
                    .unwrap_or_default();
                let source_text = self.checker.type_to_string_exported(source);
                let role = match self.tuples.lookup(self.checker, source.addr()) {
                    Some((target, TupleRole::Parameter(i))) => {
                        format!("tuple#{}@{:x}", i, target.addr() & 0xffff)
                    }
                    Some((_, TupleRole::Member)) => "member".into(),
                    None => format!(
                        "sym={}",
                        source
                            .symbol()
                            .map(|s| s.name().to_owned())
                            .unwrap_or_default()
                    ),
                };
                let source_key = format!("{source_key}[{source_text}|{role}]");
                dump.push(format!("{source_key}->{key}:{text}"));
            }
            self.dump = Some(dump.join(" ; "));
        }
        pairs.sort_unstable();
        pairs.dedup();
        let mut enc = Enc::new("mapper");
        for (source, image) in &pairs {
            enc.digest(source).digest(image);
        }
        for function in functions {
            enc.str(function);
        }
        Some(enc.finish())
    }

    /// The key of `t`, a function of the type graph reachable from it (and
    /// of `depth`), never of what was keyed before:
    ///
    /// * a type reached again on the current path is a back-reference by
    ///   distance, so a recursive type is keyed by its unfolding;
    /// * a key is memoized only when its subtree holds no back-reference
    ///   (then it is the same from every path), with the depth it reaches, so
    ///   a memoized key fails by depth exactly where keying it again would;
    /// * a subtree holding a back-reference is keyed again on every visit
    ///   (a memoized key of a node on a cycle would encode the cycle from
    ///   the node's own entry, and spliced into another entry's unfolding it
    ///   would key one regular tree two ways), within [`CYCLIC_BUDGET`].
    pub(crate) fn type_key(&mut self, t: P<Type>, depth: u32) -> Option<Digest32> {
        let address = t.addr();
        self.reach = self.reach.max(depth);
        if let Some(index) = self.stack.iter().position(|entry| *entry == address) {
            self.cyclic = true;
            let distance = (self.stack.len() - index) as u64;
            return Some(Enc::new("rec").u64(distance).finish());
        }
        if let Some(&(known, reach)) = self.memo.get(&address) {
            if depth + reach > MAX_DEPTH {
                self.path_failed = true;
                self.failure.get_or_insert_with(|| "depth".into());
                return None;
            }
            self.reach = self.reach.max(depth + reach);
            return known;
        }
        if depth > MAX_DEPTH {
            self.path_failed = true;
            self.failure.get_or_insert_with(|| "depth".into());
            return None;
        }
        self.stack.push(address);
        let outer_cyclic = std::mem::replace(&mut self.cyclic, false);
        let outer_failed = std::mem::replace(&mut self.path_failed, false);
        let outer_reach = std::mem::replace(&mut self.reach, depth);
        let mut key = self.type_key_uncached(t, depth);
        self.stack.pop();
        if self.cyclic {
            self.cyclic_work += 1;
            if self.cyclic_work > CYCLIC_BUDGET {
                self.path_failed = true;
                self.failure.get_or_insert_with(|| "cyclic-budget".into());
                key = None;
            }
        } else if !self.path_failed && (key.is_some() || !stats_enabled()) {
            self.memo.insert(address, (key, self.reach - depth));
        }
        self.cyclic |= outer_cyclic;
        self.path_failed |= outer_failed;
        self.reach = self.reach.max(outer_reach);
        if key.is_none() {
            let variant = match t.data() {
                TypeData::Intrinsic(_) => "intrinsic",
                TypeData::Literal(_) => "literal",
                TypeData::UniqueESSymbol(_) => "unique",
                TypeData::Object(_) => "object",
                TypeData::TypeReference(_) => "reference",
                TypeData::Interface(_) => "interface",
                TypeData::Tuple(_) => "tuple",
                TypeData::InstantiationExpression(_) => "instantiation-expression",
                TypeData::Mapped(_) => "mapped",
                TypeData::ReverseMapped(_) => "reverse-mapped",
                TypeData::EvolvingArray(_) => "evolving-array",
                TypeData::Union(_) => "union",
                TypeData::Intersection(_) => "intersection",
                TypeData::TypeParameter(_) => "type-parameter",
                TypeData::Index(_) => "index",
                TypeData::IndexedAccess(_) => "indexed-access",
                TypeData::TemplateLiteral(_) => "template",
                TypeData::StringMapping(_) => "string-mapping",
                TypeData::Substitution(_) => "substitution",
                TypeData::Conditional(_) => "conditional",
            };
            let symbol = if t.symbol().is_some() { "+sym" } else { "-sym" };
            self.failure
                .get_or_insert_with(|| format!("{variant}{symbol}"));
        }
        key
    }

    fn named(&mut self, tag: &str, t: P<Type>, depth: u32) -> Option<Digest32> {
        let Some(symbol) = t.symbol() else {
            self.failure
                .get_or_insert_with(|| format!("{tag}:no-symbol"));
            return None;
        };
        let Some(id) = self.symbol_id(symbol, depth + 1) else {
            self.failure
                .get_or_insert_with(|| format!("{tag}:unkeyed-symbol:{}", symbol.name()));
            return None;
        };
        // Only the flags a type is created with and that separate two types of
        // one symbol (kind, literal origins, freshness): the others are
        // computed lazily, so they would make the key depend on history.
        const IDENTITY_FLAGS: u32 = (0x7fff & !(1 << 13)) | (0x1f << 22);
        let flags = if t
            .flags
            .get()
            .intersects(tsrs_checker::types::TypeFlags::Object)
        {
            t.object_flags.get().bits() & IDENTITY_FLAGS
        } else {
            0
        };
        Some(Enc::new(tag).str(&id).u64(u64::from(flags)).finish())
    }

    fn type_key_uncached(&mut self, t: P<Type>, depth: u32) -> Option<Digest32> {
        match t.data() {
            TypeData::Intrinsic(intrinsic) => {
                // The checker's intrinsics are singletons (several share a
                // name: `any`, `never`, `undefined`), keyed by which one; the
                // only intrinsic made later is an unresolved alias's `error`,
                // one per alias and type arguments.
                if let Some(which) = singleton_type(self.checker, t) {
                    return Some(Enc::new("intrinsic").str(which).finish());
                }
                let alias = self.need(t.alias(), "intrinsic:dynamic")?;
                let symbol = self.need(alias.symbol.get(), "intrinsic:alias-no-symbol")?;
                let symbol = self.symbol_id(symbol, depth + 1);
                let symbol = self.need(symbol, "intrinsic:alias-unkeyed")?;
                let mut enc = Enc::new("error-alias");
                enc.str(intrinsic.intrinsic_name.get()).str(&symbol);
                for argument in alias.type_arguments.get() {
                    let argument = self.type_key(*argument, depth + 1)?;
                    enc.digest(&argument);
                }
                Some(enc.finish())
            }
            TypeData::Literal(literal) => {
                let mut enc = Enc::new("literal");
                match literal.value.get() {
                    Some(LiteralValue::String(value)) => enc.str("s").str(value),
                    Some(LiteralValue::Number(value)) => enc.str("n").str(&format!("{value:?}")),
                    Some(LiteralValue::Boolean(value)) => {
                        enc.str("b").str(if value { "1" } else { "0" })
                    }
                    Some(LiteralValue::BigInt(value)) => enc.str("i").str(&format!("{value:?}")),
                    None => enc.str("none"),
                };
                enc.u64(u64::from(t.flags.get().bits()));
                // Freshness is not identity: which of a value's fresh and
                // regular literal types a checker holds depends on what it
                // widened first, and the two are identical types.
                if let Some(symbol) = t.symbol() {
                    let id = self.symbol_id(symbol, depth + 1);
                    enc.str(&self.need(id, "literal:unkeyed-enum")?);
                }
                Some(enc.finish())
            }
            TypeData::UniqueESSymbol(_) => self.named("unique", t, depth),
            TypeData::TypeParameter(parameter) => {
                if let Some(target) = parameter.target.get() {
                    let target = self.type_key(target, depth + 1)?;
                    let mapper = self.need(parameter.mapper.get(), "tp-inst:no-mapper")?;
                    let mapper = self.mapper_key(mapper, depth + 1)?;
                    return Some(Enc::new("tp-inst").digest(&target).digest(&mapper).finish());
                }
                // tsrs shares one element type parameter per index across every
                // tuple target (`Checker::tuple_elements`): its position is its
                // identity, in every checker.
                if t.symbol().is_none()
                    && let Some(index) = self.checker.tuple_element_index_exported(t)
                {
                    return Some(
                        Enc::new("tuple-element-parameter")
                            .u64(index as u64)
                            .finish(),
                    );
                }
                if t.symbol().is_none()
                    && let Some((target, TupleRole::Parameter(index))) =
                        self.tuples.lookup(self.checker, t.addr())
                {
                    let target = self.type_key(target, depth + 1)?;
                    return Some(
                        Enc::new("tuple-parameter")
                            .digest(&target)
                            .u64(index as u64)
                            .finish(),
                    );
                }
                let tag = if parameter.is_this_type.get() {
                    "this"
                } else {
                    "tp"
                };
                self.named(tag, t, depth)
            }
            TypeData::Interface(_) | TypeData::Tuple(_) => {
                if let TypeData::Tuple(tuple) = t.data() {
                    let mut enc = Enc::new("tuple-target");
                    for element in tuple.element_infos.get() {
                        enc.u64(u64::from(element.flags.bits()));
                        match element.labeled_declaration.and_then(node_location) {
                            Some((path, start, end)) => {
                                enc.str(&path).u64(u64::from(start)).u64(u64::from(end));
                            }
                            None => {
                                enc.str("");
                            }
                        }
                    }
                    enc.u64(u64::from(tuple.readonly.get()));
                    return Some(enc.finish());
                }
                self.named("interface", t, depth)
            }
            TypeData::TypeReference(_) => {
                let target = self.need(t.target(), "reference:no-target")?;
                let target = self.type_key(target, depth + 1)?;
                let arguments = self.checker.get_type_arguments_exported(t);
                let mut enc = Enc::new("reference");
                enc.digest(&target);
                for argument in arguments {
                    let argument = self.type_key(*argument, depth + 1)?;
                    enc.digest(&argument);
                }
                Some(enc.finish())
            }
            TypeData::Object(_) | TypeData::Mapped(_) => {
                if let (Some(target), Some(mapper)) = (t.target(), t.mapper()) {
                    let target = self.type_key(target, depth + 1)?;
                    let mapper = self.mapper_key(mapper, depth + 1)?;
                    return Some(
                        Enc::new("instantiated")
                            .digest(&target)
                            .digest(&mapper)
                            .finish(),
                    );
                }
                if let TypeData::Mapped(mapped) = t.data() {
                    let declaration =
                        self.need(mapped.declaration.get(), "mapped:no-declaration")?;
                    let (path, start, end) =
                        self.need(node_location(declaration), "mapped:no-location")?;
                    return Some(
                        Enc::new("mapped-type")
                            .str(&path)
                            .u64(u64::from(start))
                            .u64(u64::from(end))
                            .finish(),
                    );
                }
                if let Some(which) = singleton_type(self.checker, t) {
                    return Some(Enc::new("singleton-type").str(which).finish());
                }
                self.named("anonymous", t, depth)
            }
            TypeData::Union(_) | TypeData::Intersection(_) => {
                let tag = if matches!(t.data(), TypeData::Union(_)) {
                    "union"
                } else {
                    "intersection"
                };
                let mut members = Vec::new();
                for member in t.types() {
                    members.push(self.type_key(*member, depth + 1)?);
                }
                if tag == "union" {
                    members.sort_unstable();
                }
                let mut enc = Enc::new(tag);
                for member in &members {
                    enc.digest(member);
                }
                Some(enc.finish())
            }
            TypeData::Index(index) => {
                let target = self.need(index.target.get(), "keyof:no-target")?;
                let target = self.type_key(target, depth + 1)?;
                Some(
                    Enc::new("keyof")
                        .digest(&target)
                        .u64(u64::from(index.index_flags.get().bits()))
                        .finish(),
                )
            }
            TypeData::IndexedAccess(access) => {
                let object = self.need(access.object_type.get(), "indexed:no-object")?;
                let object = self.type_key(object, depth + 1)?;
                let index = self.need(access.index_type.get(), "indexed:no-index")?;
                let index = self.type_key(index, depth + 1)?;
                Some(
                    Enc::new("indexed")
                        .digest(&object)
                        .digest(&index)
                        .u64(u64::from(access.access_flags.get().bits()))
                        .finish(),
                )
            }
            TypeData::TemplateLiteral(template) => {
                let mut enc = Enc::new("template");
                for text in template.texts.get() {
                    enc.str(text);
                }
                for member in template.types.get() {
                    let member = self.type_key(*member, depth + 1)?;
                    enc.digest(&member);
                }
                Some(enc.finish())
            }
            TypeData::StringMapping(mapping) => {
                let target = self.type_key(mapping.target.get()?, depth + 1)?;
                let symbol = self.symbol_id(t.symbol()?, depth + 1)?;
                Some(
                    Enc::new("string-mapping")
                        .str(&symbol)
                        .digest(&target)
                        .finish(),
                )
            }
            TypeData::Substitution(substitution) => {
                let base = self.type_key(substitution.base_type.get()?, depth + 1)?;
                let constraint = self.type_key(substitution.constraint.get()?, depth + 1)?;
                Some(
                    Enc::new("substitution")
                        .digest(&base)
                        .digest(&constraint)
                        .finish(),
                )
            }
            TypeData::Conditional(conditional) => {
                let root = self.need(conditional.root.get(), "conditional:no-root")?;
                let node = self.need(root.node.get(), "conditional:no-node")?;
                let (path, start, end) =
                    self.need(node_location(node), "conditional:no-location")?;
                let mut enc = Enc::new("conditional");
                enc.str(&path).u64(u64::from(start)).u64(u64::from(end));
                if let Some(mapper) = conditional.mapper.get() {
                    let mapper = self.mapper_key(mapper, depth + 1)?;
                    enc.digest(&mapper);
                }
                Some(enc.finish())
            }
            TypeData::InstantiationExpression(_)
            | TypeData::ReverseMapped(_)
            | TypeData::EvolvingArray(_) => None,
        }
    }
}

/// Which of the checker's own singleton types `t` is, by pointer: made once
/// per checker at its creation (or, for `import.meta`, on first use), so the
/// field names one type in every checker. When two fields hold one object
/// (a widening type without `strictNullChecks`), the first listed wins in
/// every checker alike.
fn singleton_type(checker: &Checker, t: P<Type>) -> Option<&'static str> {
    let address = t.addr();
    let table: [(P<Type>, &'static str); 36] = [
        (checker.any_type, "any"),
        (checker.auto_type, "auto"),
        (checker.wildcard_type, "wildcard"),
        (checker.blocked_string_type, "blocked-string"),
        (checker.error_type, "error"),
        (checker.unresolved_type, "unresolved"),
        (checker.non_inferrable_any_type, "non-inferrable-any"),
        (checker.intrinsic_marker_type, "intrinsic-marker"),
        (checker.unknown_type, "unknown"),
        (checker.undefined_type, "undefined"),
        (checker.undefined_widening_type, "undefined-widening"),
        (checker.missing_type, "missing"),
        (checker.optional_type, "optional"),
        (checker.null_type, "null"),
        (checker.null_widening_type, "null-widening"),
        (checker.string_type, "string"),
        (checker.number_type, "number"),
        (checker.bigint_type, "bigint"),
        (checker.es_symbol_type, "symbol"),
        (checker.void_type, "void"),
        (checker.never_type, "never"),
        (checker.silent_never_type, "silent-never"),
        (checker.implicit_never_type, "implicit-never"),
        (checker.unreachable_never_type, "unreachable-never"),
        (checker.non_primitive_type, "object"),
        (checker.unique_literal_type, "unique-literal"),
        (checker.empty_object_type, "empty-object"),
        (checker.empty_jsx_object_type, "empty-jsx-object"),
        (
            checker.empty_fresh_jsx_object_type,
            "empty-fresh-jsx-object",
        ),
        (checker.empty_type_literal_type, "empty-type-literal"),
        (checker.unknown_empty_object_type, "unknown-empty-object"),
        (checker.empty_generic_type, "empty-generic"),
        (checker.any_function_type, "any-function"),
        (checker.no_constraint_type, "no-constraint"),
        (checker.circular_constraint_type, "circular-constraint"),
        (checker.resolving_default_type, "resolving-default"),
    ];
    if let Some((_, which)) = table
        .iter()
        .find(|(singleton, _)| singleton.addr() == address)
    {
        return Some(which);
    }
    checker
        .deferred_global_import_meta_expression_type
        .filter(|singleton| singleton.addr() == address)
        .map(|_| "import-meta-expression")
}

/// Which checker-made singleton symbol `symbol` is: the symbol of a singleton
/// type above, or a member of the `import.meta` expression type (`meta`).
fn singleton_symbol(checker: &Checker, symbol: P<Symbol>) -> Option<&'static str> {
    if !symbol.flags().intersects(SymbolFlags::Transient) || !symbol.declarations().is_empty() {
        return None;
    }
    if checker
        .empty_type_literal_type
        .symbol()
        .is_some_and(|singleton| singleton.addr() == symbol.addr())
    {
        return Some("empty-type-literal");
    }
    let meta = checker
        .deferred_global_import_meta_expression_type?
        .symbol()?;
    if meta.addr() == symbol.addr() {
        return Some("import-meta-expression");
    }
    (symbol
        .parent()
        .is_some_and(|parent| parent.addr() == meta.addr())
        && symbol.name() == "meta")
        .then_some("import-meta-expression.meta")
}

/// The domain of a mapper: its sources, the checker singleton functions it
/// applies (in tree order: a function maps every type parameter), and the
/// type parameters an inference context has already inferred (read from the
/// cache, never fixed here: fixing is a side effect). An inference not yet
/// inferred has no key.
fn collect_sources(
    checker: &Checker,
    mapper: P<TypeMapper>,
    out: &mut Vec<P<Type>>,
    functions: &mut Vec<&'static str>,
    inferred: &mut Vec<(P<Type>, P<Type>)>,
    depth: u32,
) -> Option<()> {
    if depth > 32 {
        return None;
    }
    match mapper.data() {
        TypeMapperData::Simple { source, .. } => out.push(source),
        TypeMapperData::Array { sources, .. } | TypeMapperData::ArrayToSingle { sources, .. } => {
            out.extend_from_slice(sources);
        }
        TypeMapperData::Deferred { data } => out.extend_from_slice(data.sources),
        TypeMapperData::Merged { m1, m2 } | TypeMapperData::Composite { m1, m2 } => {
            collect_sources(checker, m1, out, functions, inferred, depth + 1)?;
            collect_sources(checker, m2, out, functions, inferred, depth + 1)?;
        }
        TypeMapperData::Function { .. } => {
            let address = mapper.addr();
            let name = [
                (checker.permissive_mapper, "permissive"),
                (checker.restrictive_mapper, "restrictive"),
                (checker.unique_literal_mapper, "unique-literal"),
                (checker.report_unreliable_mapper, "report-unreliable"),
                (checker.report_unmeasurable_mapper, "report-unmeasurable"),
            ]
            .into_iter()
            .find(|(singleton, _)| singleton.addr() == address)?
            .1;
            functions.push(name);
        }
        TypeMapperData::Inference { n, .. } => {
            for info in n.inferences.get() {
                let parameter = info.type_parameter.get()?;
                let image = info.inferred_type.get()?;
                inferred.push((parameter, image));
            }
        }
    }
    Some(())
}

pub(crate) fn pointer_signature(checker: &mut Checker, symbol: P<Symbol>) -> Vec<usize> {
    let Some(links) = checker.value_symbol_links.try_get_if_id_assigned(symbol) else {
        return vec![symbol.addr()];
    };
    let (target, mapper, containing, own) = (
        links.target(),
        links.mapper(),
        links.containing_type(),
        links.resolved_type.get(),
    );
    let mut signature = vec![target.map_or(0, |target| target.addr())];
    if let Some(mapper) = mapper {
        let mut sources = Vec::new();
        if collect_sources(
            checker,
            mapper,
            &mut sources,
            &mut Vec::new(),
            &mut Vec::new(),
            0,
        )
        .is_some()
        {
            sources.sort_by_key(|source| source.addr());
            sources.dedup_by_key(|source| source.addr());
            for source in sources {
                let image = mapper.map(checker, source);
                signature.push(source.addr());
                signature.push(image.addr());
            }
        } else {
            signature.push(mapper.addr());
        }
    } else if let Some(own) = own {
        signature.push(own.addr());
    }
    if let Some(containing) = containing {
        signature.push(containing.addr());
    }
    signature
}

/// The mapper kinds of an unkeyed symbol (measurement).
pub(crate) fn failure_reason(checker: &mut Checker, symbol: P<Symbol>) -> String {
    let Some(links) = checker.value_symbol_links.try_get_if_id_assigned(symbol) else {
        return "no-links".into();
    };
    if let Some(mapper) = links.mapper() {
        let mut kinds = Vec::new();
        mapper_kinds(mapper, &mut kinds, 0);
        return format!("mapper[{}]", kinds.join(","));
    }
    "type".into()
}

fn mapper_kinds(mapper: P<TypeMapper>, out: &mut Vec<&'static str>, depth: u32) {
    if depth > 8 {
        return;
    }
    match mapper.data() {
        TypeMapperData::Simple { .. } => out.push("simple"),
        TypeMapperData::Array { .. } => out.push("array"),
        TypeMapperData::ArrayToSingle { .. } => out.push("array-single"),
        TypeMapperData::Deferred { .. } => out.push("deferred"),
        TypeMapperData::Function { .. } => out.push("function"),
        TypeMapperData::Inference { .. } => out.push("inference"),
        TypeMapperData::Merged { m1, m2 } | TypeMapperData::Composite { m1, m2 } => {
            mapper_kinds(m1, out, depth + 1);
            mapper_kinds(m2, out, depth + 1);
        }
    }
}

fn stats_enabled() -> bool {
    std::env::var_os(ENV_STATS).is_some()
}

pub(crate) fn record(path: &std::ffi::OsStr, line: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(format!("{line}\n").as_bytes());
    }
}
