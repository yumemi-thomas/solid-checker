//! Probes: recorded reads of what the memo layer does not hold itself.
//!
//! A table that is not ported yet, or an L1 index the store does not retain,
//! is read through a probe: a question with a key whose answer the build
//! computes from the table (`SemanticLookup`, the freshly folded indexes).
//! The read is recorded as [`Input::Polled`](super::build::Input::Polled)
//! with the answer's fingerprint, and the store asks every recorded probe
//! again at the start of each build ([`super::build::MemoStore::begin`]): an
//! entry whose probe answers differently is dirty. A probe's answer is exactly
//! what the unported table says for that key, so the read is exact; it is
//! coarse only in that a file-spanning table is asked whole for its key.
//!
//! Each [`ProbeAnswer`] is plain, ordered data (never a hash container), so a
//! fingerprint of it is a function of its value.

use std::sync::Arc;

use solid_facts::core::Span;

use super::Ctx;
use super::build::BuildSchema;
use crate::base::{FileId, Hazard};

/// A question a build can ask of an unported table or an index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Probe {
    /// Some load the module census cannot resolve exists. Unit.
    UnknownLoad,
    /// The module census exposes the file's module as a namespace. File.
    Exposed,
    /// The module is on or imports an import cycle. File.
    Cyclic,
    /// Some file has the hazard. Unit.
    Hazard(Hazard),
    /// Member stability assumes a member name of an object created in a
    /// file. FileText.
    Stable,
    /// The binding a reference resolves to. FileSpan.
    BindingAt,
    /// The binding a reference resolves to, with the symbol of the name
    /// that resolved it. FileSpan.
    BindingNameAt,
    /// The function a symbol names. Text.
    FunctionForSymbol,
    /// The project class a symbol names. Text.
    ClassFor,
    /// TypeScript's declarations of a symbol. Text.
    SymbolInfo,
    /// TypeScript's symbol of the entity at a span of a file. FileSpan.
    SymbolAt,
    /// TypeScript's references to a symbol. Text.
    SymbolReferences,
    /// The last slot of the file with a path. Text.
    PathFile,
    /// A symbol and the symbols runtime redirects tie to it. Text.
    Redirects,
    /// A call the dialect resolves to `createSignal`. Site.
    CreateSignalCall,
    /// The standard-library declaration of a call. Site.
    StandardCall,
    /// The member names some assignment writes. Unit.
    AssignedNames,
    /// The assignments that write through `R.prototype`, in file order. Unit.
    PrototypeSites,
    /// Some reflective definer may define a property of a name. Text.
    DefinedReflectively,
    /// The type descriptor another entity naming a symbol carries
    /// (`smallest_contained_descriptor`'s last fallback). Text.
    SymbolDescriptor,
    /// The callability another entity naming a symbol carries
    /// (`smallest_contained_callability`'s last fallback). Text.
    SymbolCallability,
    /// The symbol an entity or, failing one, a resolved call's declaration
    /// names at a span of a file (`ProjectIndexes::method_symbol`). FileSpan.
    DeclarationSymbolAt,
}

/// The key of a probe.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum PolledKey {
    #[default]
    Unit,
    File(FileId),
    Text(Arc<str>),
    FileSpan(FileId, Span),
    FileText(FileId, Arc<str>),
    /// A call of a file: file, slot rank, ordinal.
    Site(FileId, u32, u32),
}

/// TypeScript's facts about one symbol that a computation reads.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SymbolInfo {
    /// The symbol aliases another.
    pub(crate) alias: bool,
    pub(crate) declarations: Vec<DeclarationInfo>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DeclarationInfo {
    pub(crate) kind: Arc<str>,
    pub(crate) name: Arc<str>,
    pub(crate) path: Arc<str>,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// A probe's answer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProbeAnswer {
    Flag(bool),
    /// A file and its slot rank.
    Slot(Option<(FileId, u32)>),
    /// A file, its slot rank and an ordinal in it.
    Item(Option<(FileId, u32, u32)>),
    Class(Option<(FileId, u32, Span)>),
    Symbol(Option<SymbolInfo>),
    References(Option<Vec<(Arc<str>, u64, u64)>>),
    Symbols(Vec<Arc<str>>),
    Call(Option<(Arc<str>, Arc<str>)>),
    Names(Vec<Arc<str>>),
    Sites(Vec<(FileId, u32, u32)>),
    /// A binding (file, slot rank, ordinal) and the symbol of its name.
    Binding(Option<(FileId, u32, u32, Arc<str>)>),
    /// A fact's value, rendered (`Debug`).
    Rendered(Option<Arc<str>>),
}

/// What answers probes: the build's unported tables.
pub(crate) trait Probes: Sync {
    fn probe(&self, probe: Probe, key: &PolledKey) -> ProbeAnswer;
}

/// No unported table: every probe is a flag that is off. For stores driven
/// without a project.
#[cfg(test)]
pub(crate) struct NoProbes;

#[cfg(test)]
impl Probes for NoProbes {
    fn probe(&self, _: Probe, _: &PolledKey) -> ProbeAnswer {
        ProbeAnswer::Flag(false)
    }
}

pub(crate) fn fingerprint(probes: &dyn Probes, probe: Probe, key: &PolledKey) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    probes.probe(probe, key).hash(&mut hasher);
    hasher.finish()
}

impl<'c, 'w> Ctx<'c, 'w, BuildSchema> {
    /// The answer of `probe` at `key`, recorded.
    pub(crate) fn probe(&self, probe: Probe, key: PolledKey) -> ProbeAnswer {
        use std::hash::{Hash, Hasher};
        let world = self.world();
        let answer = world.probes.probe(probe, &key);
        let mut hasher = rustc_hash::FxHasher::default();
        answer.hash(&mut hasher);
        let fingerprint = hasher.finish();
        let input = world.store.poll(probe, &key, fingerprint);
        self.record_input(input);
        answer
    }

    fn flag(&self, probe: Probe, key: PolledKey) -> bool {
        match self.probe(probe, key) {
            ProbeAnswer::Flag(flag) => flag,
            other => unreachable!("{probe:?} answers a flag, not {other:?}"),
        }
    }

    pub(crate) fn unknown_load(&self) -> bool {
        self.flag(Probe::UnknownLoad, PolledKey::Unit)
    }

    pub(crate) fn exposed(&self, file: FileId) -> bool {
        self.flag(Probe::Exposed, PolledKey::File(file))
    }

    pub(crate) fn cyclic(&self, file: FileId) -> bool {
        self.flag(Probe::Cyclic, PolledKey::File(file))
    }

    pub(crate) fn hazard(&self, hazard: Hazard) -> bool {
        self.flag(Probe::Hazard(hazard), PolledKey::Unit)
    }

    /// Whether member stability assumes `name` of an object created in `owner`.
    pub(crate) fn member_stable(&self, owner: FileId, name: &str) -> bool {
        self.flag(Probe::Stable, PolledKey::FileText(owner, Arc::from(name)))
    }

    /// ADR 0314: whether some reflective definer may define `name`
    /// (`SemanticLookup::property_may_be_defined_reflectively_as`).
    pub(crate) fn defined_reflectively(&self, name: &str) -> bool {
        self.flag(Probe::DefinedReflectively, PolledKey::Text(Arc::from(name)))
    }

    pub(crate) fn create_signal_call(&self, site: (FileId, u32, u32)) -> bool {
        self.flag(
            Probe::CreateSignalCall,
            PolledKey::Site(site.0, site.1, site.2),
        )
    }

    /// The binding a reference at `span` of `file` resolves to: its file, the
    /// file's slot rank and the binding's ordinal.
    pub(crate) fn binding_at(&self, file: FileId, span: Span) -> Option<(FileId, u32, u32)> {
        match self.probe(Probe::BindingAt, PolledKey::FileSpan(file, span)) {
            ProbeAnswer::Item(item) => item,
            other => unreachable!("binding_at answers an item, not {other:?}"),
        }
    }

    /// [`Self::binding_at`], with the symbol of the binding's name that the
    /// reference resolved to.
    pub(crate) fn binding_name_at(
        &self,
        file: FileId,
        span: Span,
    ) -> Option<(FileId, u32, u32, Arc<str>)> {
        match self.probe(Probe::BindingNameAt, PolledKey::FileSpan(file, span)) {
            ProbeAnswer::Binding(binding) => binding,
            other => unreachable!("binding_name_at answers a binding, not {other:?}"),
        }
    }

    pub(crate) fn function_for_symbol(&self, symbol: &str) -> Option<(FileId, u32, u32)> {
        match self.probe(Probe::FunctionForSymbol, PolledKey::Text(Arc::from(symbol))) {
            ProbeAnswer::Item(item) => item,
            other => unreachable!("function_for_symbol answers an item, not {other:?}"),
        }
    }

    pub(crate) fn class_for_symbol(&self, symbol: &str) -> Option<(FileId, u32, Span)> {
        match self.probe(Probe::ClassFor, PolledKey::Text(Arc::from(symbol))) {
            ProbeAnswer::Class(class) => class,
            other => unreachable!("class_for_symbol answers a class, not {other:?}"),
        }
    }

    pub(crate) fn symbol_info(&self, symbol: &str) -> Option<SymbolInfo> {
        match self.probe(Probe::SymbolInfo, PolledKey::Text(Arc::from(symbol))) {
            ProbeAnswer::Symbol(info) => info,
            other => unreachable!("symbol_info answers a symbol, not {other:?}"),
        }
    }

    /// TypeScript's symbol of the entity at `span` of `file`.
    pub(crate) fn symbol_at(&self, file: FileId, span: Span) -> Option<SymbolInfo> {
        match self.probe(Probe::SymbolAt, PolledKey::FileSpan(file, span)) {
            ProbeAnswer::Symbol(info) => info,
            other => unreachable!("symbol_at answers a symbol, not {other:?}"),
        }
    }

    pub(crate) fn symbol_references(&self, symbol: &str) -> Option<Vec<(Arc<str>, u64, u64)>> {
        match self.probe(Probe::SymbolReferences, PolledKey::Text(Arc::from(symbol))) {
            ProbeAnswer::References(references) => references,
            other => unreachable!("symbol_references answers references, not {other:?}"),
        }
    }

    /// The last slot of the analyzed file with `path`.
    pub(crate) fn path_file(&self, path: &str) -> Option<(FileId, u32)> {
        match self.probe(Probe::PathFile, PolledKey::Text(Arc::from(path))) {
            ProbeAnswer::Slot(slot) => slot,
            other => unreachable!("path_file answers a slot, not {other:?}"),
        }
    }

    pub(crate) fn redirects(&self, symbol: &str) -> Vec<Arc<str>> {
        match self.probe(Probe::Redirects, PolledKey::Text(Arc::from(symbol))) {
            ProbeAnswer::Symbols(symbols) => symbols,
            other => unreachable!("redirects answers symbols, not {other:?}"),
        }
    }

    /// The standard-library declaration of a call: its qualified name and
    /// the file that declares it.
    pub(crate) fn standard_call(&self, site: (FileId, u32, u32)) -> Option<(Arc<str>, Arc<str>)> {
        match self.probe(Probe::StandardCall, PolledKey::Site(site.0, site.1, site.2)) {
            ProbeAnswer::Call(call) => call,
            other => unreachable!("standard_call answers a call, not {other:?}"),
        }
    }

    pub(crate) fn assigned_names(&self) -> Vec<Arc<str>> {
        match self.probe(Probe::AssignedNames, PolledKey::Unit) {
            ProbeAnswer::Names(names) => names,
            other => unreachable!("assigned_names answers names, not {other:?}"),
        }
    }

    pub(crate) fn prototype_sites(&self) -> Vec<(FileId, u32, u32)> {
        match self.probe(Probe::PrototypeSites, PolledKey::Unit) {
            ProbeAnswer::Sites(sites) => sites,
            other => unreachable!("prototype_sites answers sites, not {other:?}"),
        }
    }
}
