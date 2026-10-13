//! Who resolved what: the resolution edges a certification's dependency
//! environment states.
//!
//! The environment names every installed package the proof read. On its own
//! that set cannot say *which* package read each one, so admission had to
//! demand that every lookup of each name, from every located package, reach the
//! same entry -- and a pnpm tree, whose `.pnpm/node_modules` hoists some version
//! of nearly everything into every package's view, never satisfies that, not
//! even the tree the certification ran in. Measured on kobalte core:
//! `vite-plugin-solid@3.0.0-next.5` read its own `merge-anything@5.1.7` and was
//! refused in the same tree because another located package sees the hoisted
//! 6.0.6.
//!
//! So each entry records the lookup that reached it: the bare name looked up,
//! and the importer -- the certified package, or another entry -- it was looked
//! up from. The edges are the adapter's record of the lookups its closure walk
//! made (`resolvedFrom` on each declaration source) and, on the graph lanes,
//! the graph's own parent-to-dependency edges. They are joined here to the
//! authenticated identities by installed location.

use std::collections::{BTreeMap, BTreeSet};

use super::policy2_receipt::{
    DependencyEnvironmentEntry, EnvironmentImporter, environment_edges_are_rooted,
};

/// One lookup the adapter recorded: the bare package name `specifier`, looked
/// up from the package installed at `importer_package_root`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceResolutionEdge {
    pub importer_package_root: String,
    pub specifier: String,
}

/// One package an environment's edges can reach or start from: its identity,
/// where it is installed, and the lookups that reached it.
#[derive(Clone, Debug)]
pub(super) struct LocatedEnvironmentPackage {
    pub(super) entry: DependencyEnvironmentEntry,
    pub(super) roots: Vec<String>,
    pub(super) resolved_from: Vec<SourceResolutionEdge>,
}

/// The installed location a path names, compared by real path so a pnpm
/// symlink and its store target are one location. A path that does not exist
/// (a test tree, or an importer already removed) is compared as written.
fn location(path: &str) -> String {
    std::fs::canonicalize(path).map_or_else(
        |_| path.trim_end_matches(['/', '\\']).to_owned(),
        |real| real.to_string_lossy().into_owned(),
    )
}

/// `entries` with every lookup that reached each one, or `None` when some
/// entry has no recorded lookup from the certified package or from another
/// entry.
///
/// `None` is not a refusal: the caller then states the environment without
/// edges, which is read by the strict all-lookups rule -- sound, only stricter.
/// A lookup whose importer is not in the environment (a package the census did
/// not admit) is dropped rather than trusted, because no consumer could replay
/// it; if that leaves an entry unreached, the whole environment falls back.
pub(super) fn environment_with_edges(
    certified_roots: &[String],
    entries: &BTreeSet<DependencyEnvironmentEntry>,
    located: &[LocatedEnvironmentPackage],
) -> Option<Vec<DependencyEnvironmentEntry>> {
    if entries.is_empty() {
        return Some(Vec::new());
    }
    let member = |entry: &DependencyEnvironmentEntry| {
        entries
            .iter()
            .any(|candidate| candidate.same_package(entry))
    };
    let mut importers = BTreeMap::<String, EnvironmentImporter>::new();
    for root in certified_roots {
        importers.insert(location(root), EnvironmentImporter::Certified);
    }
    // A location two different packages claim is not an importer anyone can
    // name; it is dropped for every one of them.
    let mut ambiguous = BTreeSet::new();
    for package in located.iter().filter(|package| member(&package.entry)) {
        let importer = package.entry.as_importer();
        for root in &package.roots {
            let at = location(root);
            match importers.get(&at) {
                Some(EnvironmentImporter::Certified) => {}
                Some(existing) if *existing != importer => {
                    ambiguous.insert(at);
                }
                Some(_) => {}
                None => {
                    importers.insert(at, importer.clone());
                }
            }
        }
    }
    for at in &ambiguous {
        importers.remove(at);
    }
    let mut edged = BTreeSet::new();
    for package in located.iter().filter(|package| member(&package.entry)) {
        let own = package.entry.as_importer();
        for edge in &package.resolved_from {
            let Some(importer) = importers.get(&location(&edge.importer_package_root)) else {
                continue;
            };
            if *importer == own || edge.specifier.is_empty() {
                continue;
            }
            edged.insert(
                package
                    .entry
                    .without_edge()
                    .resolved_from(importer.clone(), edge.specifier.clone()),
            );
        }
    }
    let result = edged.into_iter().collect::<Vec<_>>();
    let covered = entries
        .iter()
        .all(|entry| result.iter().any(|edged| edged.same_package(entry)));
    (covered && environment_edges_are_rooted(&result)).then_some(result)
}

/// The bare package name a specifier addresses: `@scope/name` or `name`.
pub(super) fn package_name_of_specifier(specifier: &str) -> String {
    let mut segments = specifier.split('/');
    match (segments.next(), segments.next()) {
        (Some(scope), Some(name)) if scope.starts_with('@') => format!("{scope}/{name}"),
        (Some(name), _) => name.to_owned(),
        _ => specifier.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_certification::validate_dependency_environment;

    fn package(name: &str, version: &str) -> DependencyEnvironmentEntry {
        DependencyEnvironmentEntry::package(name, version, format!("sha512-{name}-{version}"))
    }

    fn located(
        entry: &DependencyEnvironmentEntry,
        root: &str,
        from: &[(&str, &str)],
    ) -> LocatedEnvironmentPackage {
        LocatedEnvironmentPackage {
            entry: entry.clone(),
            roots: vec![root.to_owned()],
            resolved_from: from
                .iter()
                .map(|(importer, specifier)| SourceResolutionEdge {
                    importer_package_root: (*importer).to_owned(),
                    specifier: (*specifier).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn edges_join_recorded_lookups_to_authenticated_identities_by_location() {
        let merge = package("merge-anything", "5.1.7");
        let is_what = package("is-what", "4.1.8");
        let entries = BTreeSet::from([merge.clone(), is_what.clone()]);
        let edged = environment_with_edges(
            &["/t/.pnpm/vps/node_modules/vite-plugin-solid".to_owned()],
            &entries,
            &[
                located(
                    &merge,
                    "/t/.pnpm/ma/node_modules/merge-anything",
                    &[(
                        "/t/.pnpm/vps/node_modules/vite-plugin-solid",
                        "merge-anything",
                    )],
                ),
                located(
                    &is_what,
                    "/t/.pnpm/iw/node_modules/is-what",
                    &[
                        ("/t/.pnpm/ma/node_modules/merge-anything", "is-what"),
                        // An importer outside the environment is not an edge.
                        ("/t/.pnpm/elsewhere/node_modules/other", "is-what"),
                    ],
                ),
            ],
        )
        .expect("every entry is reached");
        assert_eq!(
            edged,
            vec![
                is_what
                    .clone()
                    .resolved_from(merge.as_importer(), "is-what"),
                merge
                    .clone()
                    .resolved_from(EnvironmentImporter::Certified, "merge-anything"),
            ]
        );
        validate_dependency_environment(&edged).expect("canonical and rooted");
    }

    #[test]
    fn an_entry_no_recorded_lookup_reaches_falls_back_to_the_strict_form() {
        let merge = package("merge-anything", "5.1.7");
        let entries = BTreeSet::from([merge.clone()]);
        assert_eq!(
            environment_with_edges(
                &["/t/root".to_owned()],
                &entries,
                &[located(
                    &merge,
                    "/t/ma",
                    &[("/t/unrelated", "merge-anything")]
                )],
            ),
            None
        );
        assert_eq!(
            environment_with_edges(&["/t/root".to_owned()], &entries, &[]),
            None
        );
    }

    #[test]
    fn two_importers_may_reach_two_versions_of_one_name() {
        let old = package("merge-anything", "5.1.7");
        let new = package("merge-anything", "6.0.6");
        let other = package("other", "1.0.0");
        let entries = BTreeSet::from([old.clone(), new.clone(), other.clone()]);
        let edged = environment_with_edges(
            &["/t/root".to_owned()],
            &entries,
            &[
                located(&old, "/t/old", &[("/t/root", "merge-anything")]),
                located(&other, "/t/other", &[("/t/root", "other")]),
                located(&new, "/t/new", &[("/t/other", "merge-anything")]),
            ],
        )
        .expect("both copies are reached by their own importer");
        assert_eq!(edged.len(), 3);
        validate_dependency_environment(&edged).expect("two copies are expressible with edges");
    }

    #[test]
    fn specifier_names_are_bare_package_names() {
        assert_eq!(package_name_of_specifier("@a/b/c"), "@a/b");
        assert_eq!(package_name_of_specifier("a/b"), "a");
        assert_eq!(package_name_of_specifier("a"), "a");
    }
}
