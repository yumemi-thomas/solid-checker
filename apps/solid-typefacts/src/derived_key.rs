//! Checker-independent keys for derived symbols (instant-edits P2, prototype).
//!
//! A derived symbol (an instantiation, a transient clone, a mapped or a
//! synthetic property) is keyed by what it is derived from and by the exact
//! type context the checker derived it in, never by erasure: two symbols one
//! checker keeps apart never share a key (`Foo<number>.x` and
//! `Foo<string>.x` differ), and two checkers deriving the same symbol give it
//! the same key, whatever each checked first.
//!
//! * Instantiated (`target` and `mapper`): when instantiation cannot affect
//!   the root (its type could contain no type variable), the checker itself
//!   returns the root once that type is resolved, so the key is the root's;
//!   otherwise `inst(root, mapper)`.
//! * Clone (`target`, no `mapper`): `clone(root, type of the clone)`.
//! * Mapped property: `mapped(name, containing mapped type)`.
//! * Synthetic union/intersection property outside the `Set` key:
//!   `synth(name, containing type)`.
//!
//! Types are keyed structurally (`type_key`): intrinsics and literals by value,
//! named and anonymous types by their declaration's key, references by target
//! and type arguments, instantiated anonymous types by target and mapper,
//! unions and intersections as sorted key sets. Anything else, a cycle, or a
//! depth past [`MAX_DEPTH`] has no key (fail closed: the caller falls back).

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

/// Why a derived symbol has no key (prototype statistics).
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
}

pub(crate) struct Keyer<'a, F: Fn(&DurableRef) -> bool> {
    pub checker: &'a mut Checker,
    pub exported: F,
    pub memo: &'a mut HashMap<usize, Option<Digest32>>,
    pub tuples: &'a mut TupleIndex,
    /// Types being keyed, outermost first: a revisit is a back-reference.
    stack: Vec<usize>,
    /// The outermost stack index a back-reference in the current subtree
    /// points to (`usize::MAX`: none).
    open_min: usize,
    /// Prototype: the type variant (and depth) of the first unkeyable part.
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
        memo: &'a mut HashMap<usize, Option<Digest32>>,
        tuples: &'a mut TupleIndex,
    ) -> Self {
        Self {
            checker,
            exported,
            memo,
            tuples,
            stack: Vec::new(),
            open_min: usize::MAX,
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
        if depth > MAX_DEPTH {
            return (None, Outcome::NoType);
        }
        let name = normalized_name(symbol.name()).into_owned();
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
                if root.flags().intersects(SymbolFlags::SetAccessor) {
                    return (None, Outcome::NoType);
                }
                let root_type = self.checker.get_type_of_symbol_exported(root);
                if !self
                    .checker
                    .could_contain_type_variables_exported(root_type)
                {
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

    pub(crate) fn type_key(&mut self, t: P<Type>, depth: u32) -> Option<Digest32> {
        let address = t.addr();
        if let Some(index) = self.stack.iter().position(|entry| *entry == address) {
            // A recursive type: a back-reference by distance, which keys the
            // same regular tree the same way from any entry point.
            self.open_min = self.open_min.min(index);
            let distance = (self.stack.len() - index) as u64;
            return Some(Enc::new("rec").u64(distance).finish());
        }
        if let Some(known) = self.memo.get(&address) {
            return *known;
        }
        if depth > MAX_DEPTH {
            self.failure.get_or_insert_with(|| "depth".into());
            return None;
        }
        let index = self.stack.len();
        self.stack.push(address);
        let outer = std::mem::replace(&mut self.open_min, usize::MAX);
        let key = self.type_key_uncached(t, depth);
        self.stack.pop();
        let inner = self.open_min;
        // Closed at or inside this type: the key is a function of the type
        // alone. Open to an ancestor: it depends on the path, so not memoized.
        if inner >= index {
            self.memo.insert(address, key);
        }
        self.open_min = outer.min(if inner < index { inner } else { usize::MAX });
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
            TypeData::Intrinsic(intrinsic) => Some(
                Enc::new("intrinsic")
                    .str(intrinsic.intrinsic_name.get())
                    .u64(u64::from(t.flags.get().bits()))
                    .finish(),
            ),
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

/// Why `type_key` failed for `t`, by the first unkeyable part (prototype).
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

pub(crate) fn record(path: &std::ffi::OsStr, line: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}
