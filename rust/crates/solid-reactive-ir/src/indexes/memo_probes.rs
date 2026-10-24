//! The unported tables as memo probes ([`crate::base::memo::probes`]): what
//! this build's lookup answers to the questions a memo computation records.

use super::*;
use crate::base::memo::probes::{
    DeclarationInfo, PolledKey, Probe, ProbeAnswer, Probes, SymbolInfo,
};
use crate::base::{FileId, Hazard, MemberName, MemberRole};

thread_local! {
    /// Set while a probe is being asked: a probe that reads a memo would
    /// begin the build from inside its own invalidation.
    static PROBING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the current thread is answering a probe.
pub(crate) fn probing() -> bool {
    PROBING.with(std::cell::Cell::get)
}

/// One file's binding resolutions ([`SemanticLookup::binding_resolution_rows`]).
pub(crate) type BindingResolutionRows = Vec<((u64, u64), (FileId, u32, u32, SymbolId))>;

impl SemanticLookup<'_> {
    /// The slot of `(id, rank)` in the build's files, by the memo table.
    fn memo_slot(&self, id: FileId, rank: u32) -> Option<usize> {
        self.memos.table().slots_of(id).get(rank as usize).copied()
    }

    /// A file of the lookup's own table as the memo table names it.
    fn memo_id(&self, id: FileId) -> Option<FileId> {
        self.memos.table().id_of(self.file_table.path_of(id))
    }

    pub(crate) fn memo_slot_of(&self, slot: usize) -> (FileId, u32) {
        let table = self.memos.table();
        (
            table.slot_id(slot),
            u32::try_from(table.slot_rank(slot)).expect("fewer than 2^32 slots of one path"),
        )
    }

    /// Every binding resolution the lookup holds for spans of `path`
    /// ([`Self::binding_at_reference`]'s table): `((start, end), (file, slot
    /// rank, binding ordinal, symbol))`, sorted by span. One file's rows, for
    /// the resolver's per-file adapter (S7b).
    pub(crate) fn binding_resolution_rows(&self, path: &str) -> BindingResolutionRows {
        let mut rows = self
            .bindings_by_reference()
            .get(path)
            .map(|references| {
                references
                    .iter()
                    .map(|(span, resolution)| {
                        let (file, rank) = self.memo_slot_of(resolution.file);
                        (
                            *span,
                            (
                                file,
                                rank,
                                u32::try_from(resolution.binding)
                                    .expect("fewer than 2^32 bindings"),
                                resolution.symbol.clone(),
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        rows.sort_unstable_by_key(|(span, _)| *span);
        rows
    }

    fn ask(&self, probe: Probe, key: &PolledKey) -> ProbeAnswer {
        let path = |id: FileId| self.memos.table().path_of(id);
        match (probe, key) {
            (Probe::UnknownLoad, _) => ProbeAnswer::Flag(self.module_census().has_unknown_load()),
            (Probe::Exposed, PolledKey::File(id)) => {
                ProbeAnswer::Flag(self.module_census().is_exposed(path(*id)))
            }
            (Probe::Cyclic, PolledKey::File(id)) => {
                ProbeAnswer::Flag(self.module_census().is_cyclic(path(*id)))
            }
            (Probe::Hazard(hazard), _) => ProbeAnswer::Flag(self.hazard_present(hazard)),
            (Probe::Stable, PolledKey::FileText(id, name)) => ProbeAnswer::Flag(matches!(
                self.member_stability()
                    .state_of(self.facts, path(*id), name),
                crate::interproc::accessor_provenance::Stability::Assumed
            )),
            (Probe::BindingAt, PolledKey::FileSpan(id, span)) => ProbeAnswer::Item(
                self.binding_resolution_at(path(*id), *span)
                    .map(|resolution| {
                        let (id, rank) = self.memo_slot_of(resolution.file);
                        (
                            id,
                            rank,
                            u32::try_from(resolution.binding).expect("fewer than 2^32 bindings"),
                        )
                    }),
            ),
            (Probe::BindingNameAt, PolledKey::FileSpan(id, span)) => ProbeAnswer::Binding(
                self.binding_resolution_at(path(*id), *span)
                    .map(|resolution| {
                        let (id, rank) = self.memo_slot_of(resolution.file);
                        (
                            id,
                            rank,
                            u32::try_from(resolution.binding).expect("fewer than 2^32 bindings"),
                            Arc::from(resolution.symbol.as_str()),
                        )
                    }),
            ),
            (Probe::FunctionForSymbol, PolledKey::Text(symbol)) => {
                ProbeAnswer::Item(match self.functions_by_symbol().get(symbol.as_ref()) {
                    Some(SymbolFunction::Resolved { file, function }) => {
                        let (id, rank) = self.memo_slot_of(*file);
                        Some((
                            id,
                            rank,
                            u32::try_from(*function).expect("fewer than 2^32 functions"),
                        ))
                    }
                    _ => None,
                })
            }
            (Probe::ClassFor, PolledKey::Text(symbol)) => {
                ProbeAnswer::Class(self.class_for_symbol(symbol).and_then(|(file, class)| {
                    let id = self.memos.table().id_of(file.path.as_str())?;
                    Some((id, 0, class.span))
                }))
            }
            (Probe::SymbolInfo, PolledKey::Text(symbol)) => {
                ProbeAnswer::Symbol(self.symbols_by_id().get(symbol.as_ref()).map(symbol_info))
            }
            (Probe::SymbolAt, PolledKey::FileSpan(id, span)) => ProbeAnswer::Symbol(
                self.symbol_at(path(*id), *span)
                    .map(|symbol| symbol_info(&symbol)),
            ),
            (Probe::SymbolReferences, PolledKey::Text(symbol)) => ProbeAnswer::References(
                self.symbol_references_if_present(symbol).map(|references| {
                    references
                        .into_iter()
                        .map(|location| {
                            (
                                Arc::clone(&location.path),
                                location.start_byte,
                                location.end_byte,
                            )
                        })
                        .collect()
                }),
            ),
            (Probe::PathFile, PolledKey::Text(file_path)) => ProbeAnswer::Slot(
                self.file_table
                    .last_slot(file_path)
                    .map(|slot| self.memo_slot_of(slot)),
            ),
            (Probe::Redirects, PolledKey::Text(symbol)) => {
                let mut symbols = crate::object_members::receiver_symbols(self.facts, symbol)
                    .into_iter()
                    .map(|symbol| Arc::<str>::from(symbol.as_str()))
                    .collect::<Vec<_>>();
                symbols.sort_unstable();
                ProbeAnswer::Symbols(symbols)
            }
            (Probe::CreateSignalCall, PolledKey::Site(id, rank, ordinal)) => {
                ProbeAnswer::Flag(self.memo_slot(*id, *rank).is_some_and(|slot| {
                    matches!(
                        self.primitives(&self.facts.files[slot])
                            .calls
                            .get(*ordinal as usize),
                        Some(Some(crate::PrimitiveName::Known(
                            solid_dialect::Primitive::CreateSignal,
                            _
                        )))
                    )
                }))
            }
            (Probe::StandardCall, PolledKey::Site(id, rank, ordinal)) => {
                ProbeAnswer::Call(self.memo_slot(*id, *rank).and_then(|slot| {
                    let file = &self.facts.files[slot];
                    let call = file.ast.calls.get(*ordinal as usize)?;
                    let declaration =
                        self.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
                    Some((
                        Arc::clone(&declaration.qualified_name),
                        Arc::clone(&declaration.source_file),
                    ))
                }))
            }
            (Probe::DefinedReflectively, PolledKey::Text(name)) => {
                ProbeAnswer::Flag(self.property_may_be_defined_reflectively_as(name))
            }
            (Probe::GlobalWritten, PolledKey::Text(name)) => {
                ProbeAnswer::Flag(global_identifier_written(self.facts, name))
            }
            (Probe::SymbolUses, PolledKey::Text(symbol)) => {
                let index = self.base.symbol_uses(|| self.base_views());
                let mut uses = index
                    .ranked_values(symbol)
                    .filter_map(|(file, rank, value)| {
                        Some((
                            self.memo_id(file)?,
                            u32::try_from(rank).expect("fewer than 2^32 slots of one path"),
                            *value,
                        ))
                    })
                    .collect::<Vec<_>>();
                uses.sort_unstable();
                ProbeAnswer::Uses(uses)
            }
            (Probe::Spellings, PolledKey::Text(name)) => {
                let index = self.base.spellings(|| self.base_views());
                let mut spellings = index
                    .ranked_values(name)
                    .filter_map(|(file, rank, value)| {
                        Some((
                            self.memo_id(file)?,
                            u32::try_from(rank).expect("fewer than 2^32 slots of one path"),
                            *value,
                        ))
                    })
                    .collect::<Vec<_>>();
                spellings.sort_unstable();
                ProbeAnswer::Spellings(spellings)
            }
            (Probe::AssignedNames, _) => {
                let members = self.base_members();
                let mut names = members
                    .keys()
                    .filter_map(|name| match name {
                        MemberName::Named(name) => Some(name),
                        MemberName::Unnamed => None,
                    })
                    .filter(|name| {
                        members
                            .values(&MemberName::Named(Arc::clone(name)))
                            .any(|(_, used)| used.role == MemberRole::Assigned)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                names.sort_unstable();
                ProbeAnswer::Names(names)
            }
            (Probe::PrototypeSites, _) => {
                let mut sites = Vec::new();
                for (slot, (_, entries)) in self.slot_entries(self.base_hazards()).enumerate() {
                    let (id, rank) = self.memo_slot_of(slot);
                    for (hazard, member) in entries {
                        if *hazard == Hazard::PrototypeOwnerSite {
                            sites.push((id, rank, *member));
                        }
                    }
                }
                ProbeAnswer::Sites(sites)
            }
            (probe, key) => unreachable!("probe {probe:?} has no key {key:?}"),
        }
    }
}

impl Probes for SemanticLookup<'_> {
    fn probe(&self, probe: Probe, key: &PolledKey) -> ProbeAnswer {
        let outer = PROBING.with(|probing| probing.replace(true));
        let answer = self.ask(probe, key);
        PROBING.with(|probing| probing.set(outer));
        answer
    }
}

fn symbol_info(symbol: &solid_facts::TypeScriptSymbol<'_>) -> SymbolInfo {
    SymbolInfo {
        alias: !symbol.alias_target().is_empty(),
        declarations: symbol
            .declarations()
            .iter()
            .map(|declaration| DeclarationInfo {
                kind: Arc::clone(&declaration.kind),
                name: Arc::clone(&declaration.name),
                path: Arc::clone(&declaration.location.path),
                start: declaration.location.start_byte,
                end: declaration.location.end_byte,
            })
            .collect(),
    }
}
