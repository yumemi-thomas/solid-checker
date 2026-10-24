//! The exact incremental base of the Reactive IR (P5 study §3.2, L0 and L1).
//!
//! **L0** ([`file_table`]): every analyzed path is a [`FileId`]; a build's
//! [`FileTable`] maps ids to their slots in `ProjectFacts::files`, a file's
//! [`FileItems`] number its functions, calls, JSX elements and bindings, and a
//! [`FileView`] is everything one file contributes: its own facts and its own
//! slices of the TypeFacts tables.
//!
//! **L1** ([`index`], [`contributions`]): each base index is
//! `key -> [(FileId, values)]`, folded from per-file contributions. A
//! contribution function has the type `fn(&FileView, &BuildConstants) ->
//! Vec<(K, C)>`: no project lookup, so by signature it cannot read another
//! file. Replacing a file's contribution ([`BaseIndex::update`]) gives the
//! same index a refold gives, and returns the exact set of keys whose values
//! changed (the Δ a later recorded-read layer invalidates from).
//!
//! The indexes:
//! - references by symbol (TypeFacts references, per file);
//! - calls and JSX elements by the symbol their callee or tag names;
//! - imports, re-exports, namespace imports, `import =` and module loads by
//!   specifier;
//! - member reads, writes and assignments by name;
//! - module hazards and module-level flags (eval, reflective definition and
//!   mutation candidates, prototype writes, synchronous dispatch, unmodeled
//!   loads, top-level await, exports), read as booleans;
//! - reflective-definer keys;
//! - item key spaces (every function, call, JSX element and binding).
//!
//! Each index is folded on first read. A build reads only freshly folded
//! indexes; [`retained`] keeps a copy between builds in shadow mode and proves
//! on every edit that updating it equals refolding (no reuse is enabled).

pub(crate) mod contributions;
pub(crate) mod file_table;
pub(crate) mod index;
pub(crate) mod retained;
#[cfg(test)]
mod tests;

use std::sync::{Arc, OnceLock};

use solid_facts::FileFacts;
use solid_facts::core::Span;

pub(crate) use contributions::{
    BuildConstants, CalleeSite, DefinerKey, Hazard, MemberName, MemberUse, ModuleEdge, ModuleKey,
};
pub(crate) use file_table::{FileId, FileTable, FileView, ItemKind, ViewSources};
pub(crate) use index::{BaseIndex, FileContribution};

/// A contribution function.
pub(crate) type Contribute<K, C> = fn(&FileView<'_>, &BuildConstants<'_>) -> Vec<(K, C)>;

/// The views of one build's files over one [`FileTable`].
#[derive(Clone, Copy)]
pub(crate) struct Views<'a> {
    pub(crate) table: &'a FileTable,
    pub(crate) files: &'a [FileFacts],
    pub(crate) sources: ViewSources<'a>,
    pub(crate) constants: BuildConstants<'a>,
}

impl<'a> Views<'a> {
    pub(crate) fn view(&self, slot: usize) -> FileView<'a> {
        FileView::new(&self.files[slot], self.sources)
    }

    /// The contribution of `id`: its slots' entries, in file order.
    pub(crate) fn contribution<K, C>(
        &self,
        id: FileId,
        contribute: Contribute<K, C>,
    ) -> FileContribution<K, C> {
        FileContribution::from_slots(
            self.table
                .slots_of(id)
                .iter()
                .map(|slot| contribute(&self.view(*slot), &self.constants)),
        )
    }

    /// Every file's contribution, folded.
    pub(crate) fn fold<K, C>(&self, contribute: Contribute<K, C>) -> BaseIndex<K, C>
    where
        K: Eq + std::hash::Hash + Clone + Send,
        C: PartialEq + Send,
    {
        let slots = (0..self.files.len()).collect::<Vec<_>>();
        let mut per_slot = crate::pipeline::parallel_slice_results(&slots, |slot| {
            contribute(&self.view(*slot), &self.constants)
        })
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
        BaseIndex::fold(self.table.ids().iter().map(|id| {
            (
                *id,
                FileContribution::from_slots(
                    self.table
                        .slots_of(*id)
                        .iter()
                        .map(|slot| per_slot[*slot].take().expect("each slot is read once")),
                ),
            )
        }))
    }
}

macro_rules! base_indexes {
    (
        read: { $($(#[$doc:meta])* $name:ident: $key:ty => $value:ty = $contribute:path;)* }
        unread: { $($(#[$udoc:meta])* $uname:ident: $ukey:ty => $uvalue:ty = $ucontribute:path;)* }
    ) => {
        /// The L1 indexes a build reads, each folded on first read. The
        /// unread ones (no consumer has moved onto them yet) are folded only
        /// by [`AllIndexes`].
        #[derive(Default)]
        pub(crate) struct BaseIndexes {
            $($name: OnceLock<BaseIndex<$key, $value>>,)*
        }

        impl BaseIndexes {
            $(
                $(#[$doc])*
                pub(crate) fn $name<'v>(&self, views: impl FnOnce() -> Views<'v>) -> &BaseIndex<$key, $value> {
                    self.$name.get_or_init(|| views().fold($contribute))
                }
            )*
        }

        /// Every L1 index, folded: what [`retained`] keeps and compares, and
        /// what the property tests check.
        #[derive(Clone, PartialEq)]
        pub(crate) struct AllIndexes {
            $($(#[$doc])* pub(crate) $name: BaseIndex<$key, $value>,)*
            $($(#[$udoc])* pub(crate) $uname: BaseIndex<$ukey, $uvalue>,)*
        }

        impl AllIndexes {
            pub(crate) fn fold(views: &Views<'_>) -> Self {
                Self {
                    $($name: views.fold($contribute),)*
                    $($uname: views.fold($ucontribute),)*
                }
            }

            /// Replaces the contributions of `changed` (recomputed from
            /// `views`, in parallel) and of `removed`, returning each index's
            /// Δ, rendered.
            pub(crate) fn update(
                &mut self,
                views: &Views<'_>,
                changed: &[FileId],
                removed: &[FileId],
            ) -> Vec<(&'static str, Vec<String>)> {
                let mut deltas = Vec::new();
                $(
                    let delta = self.$name.update(
                        crate::pipeline::parallel_slice_results(changed, |id| {
                            (*id, Some(views.contribution(*id, $contribute)))
                        })
                        .into_iter()
                        .chain(removed.iter().map(|id| (*id, None))),
                    );
                    deltas.push((stringify!($name), delta.iter().map(|key| format!("{key:?}")).collect()));
                )*
                $(
                    let delta = self.$uname.update(
                        crate::pipeline::parallel_slice_results(changed, |id| {
                            (*id, Some(views.contribution(*id, $ucontribute)))
                        })
                        .into_iter()
                        .chain(removed.iter().map(|id| (*id, None))),
                    );
                    deltas.push((stringify!($uname), delta.iter().map(|key| format!("{key:?}")).collect()));
                )*
                deltas
            }

            /// Each index's keys whose values differ between `self` and
            /// `other`, rendered, by index (in [`Self::update`]'s order).
            pub(crate) fn changed_keys(&self, other: &Self) -> Vec<(&'static str, Vec<String>)> {
                vec![
                    $((
                        stringify!($name),
                        self.$name.changed_keys(&other.$name).iter().map(|key| format!("{key:?}")).collect(),
                    ),)*
                    $((
                        stringify!($uname),
                        self.$uname.changed_keys(&other.$uname).iter().map(|key| format!("{key:?}")).collect(),
                    ),)*
                ]
            }

            /// The name of the first index that differs from `other`'s.
            pub(crate) fn first_difference(&self, other: &Self) -> Option<&'static str> {
                $(
                    if self.$name != other.$name {
                        return Some(stringify!($name));
                    }
                )*
                $(
                    if self.$uname != other.$uname {
                        return Some(stringify!($uname));
                    }
                )*
                None
            }
        }
    };
}

base_indexes! {
    read: {
        /// Module hazards and module-level flags.
        hazards: Hazard => u32 = contributions::hazards;
    }
    unread: {
        /// Module edges by specifier.
        modules: ModuleKey => ModuleEdge = contributions::modules;
        /// Member reads, writes and assignments by name.
        members: MemberName => MemberUse = contributions::members;
        /// Reflective-definer keys.
        definers: DefinerKey => () = contributions::definers;
        /// References by symbol.
        references: Arc<str> => Span = contributions::references;
        /// Calls and JSX elements by the symbol of their callee or tag.
        callees: Arc<str> => CalleeSite = contributions::callees;
        /// Item key spaces.
        items: ItemKind => Span = contributions::items;
    }
}
