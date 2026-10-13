//! The one rule for admitting an acceptance by artifact (ADR 0123).
//!
//! An acceptance -- a project catalog entry or an authored contract -- applies
//! to a project only when that project's own installed tree reproduces what
//! the acceptance was proven about: the artifact, its files and the
//! environment the proof read. [`admit_by_artifact`] is that rule, and
//! [`admission_refusals`] replays it to say why an acceptance was not
//! admitted.
//!
//! **What makes an acceptance applicable is the artifact, not the importer.**
//! `policy2_artifact_acceptance_root` commits to a package's name, version,
//! tarball integrity, requested entrypoint and sorted export conditions -- and
//! to no importer and no path. A project whose installed bytes reproduce that
//! root demonstrably resolved the same published artifact the contract was
//! proven about, whatever file imported it.
//!
//! **And the environment the proof read.** Two certifications of the same
//! bytes against different environments are two different acceptances, so
//! admission resolves every entry of the stated environment from the imported
//! package's own installed location, the way Node would, and compares name,
//! manifest version and lockfile integrity. A missing, different or unstatable
//! dependency refuses the acceptance.
//!
//! **The compiled-in tier is retired** (ADR 0228). It was the third supply of
//! acceptances this rule served. A receipt that cites one of its acceptances
//! (ADR 0151) rests on a claim no build carries any more, so it is refused.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    contract_certification::{
        CitedAcceptance, DependencyEnvironmentEntry, EnvironmentImporter,
        dependency_environment_states_edges, policy2_artifact_acceptance_root_for_identity,
        policy2_dependency_environment_root,
    },
    contract_interface::{
        AuthenticCase, InstalledArtifactIdentity, ResolvedTargetIdentity, admissible_cases,
        declared_conditions,
    },
};
use sha2::{Digest as _, Sha256};

#[cfg(test)]
mod tests;

/// The first compiled-in acceptance `citations` names, described; `None` for
/// an empty list (ADR 0151, ADR 0228).
///
/// The compiled-in tier is retired, so no build carries any acceptance a
/// receipt could have cited: a contract built on a cited claim is refused
/// everywhere, never trusted on the citation's word.
pub(crate) fn withdrawn_compiled_in_citation(citations: &[CitedAcceptance]) -> Option<String> {
    citations.first().map(describe_citation)
}

fn describe_citation(citation: &CitedAcceptance) -> String {
    format!(
        "{}@{} (contract {}, receipt {})",
        citation.package_name,
        citation.package_version,
        citation.semantic_digest,
        citation.receipt_digest
    )
}

/// The key an acceptance is indexed and admitted under by artifact, from any
/// supply: the acceptance root and the environment its proof read, together.
///
/// Two certifications of one artifact in different environments are two
/// acceptances; keyed by the artifact alone, `AcceptedContractIndex` would drop
/// both as a conflict, or -- across supplies -- hand one supply's environment
/// to the other's consumer. Two entries about the same artifact in the same
/// environment get the same key, which is right: they are the same statement.
pub(crate) fn environment_acceptance_identity(
    acceptance_root: &str,
    environment: &[DependencyEnvironmentEntry],
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:accepted-bundle-identity:v1");
    for field in [
        acceptance_root,
        policy2_dependency_environment_root(environment).as_str(),
    ] {
        hash.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
        hash.update(field.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}

/// Whether this project's installed tree reproduces an acceptance's dependency
/// environment, resolved from the installed location of the package the
/// specifier (the first argument) names.
///
/// The native answer is `diagnostics`' filesystem walk over
/// [`environment_is_installed`]. A host with no filesystem answers `true` only
/// for the empty environment.
pub type InstalledEnvironment<'a> = dyn Fn(&str, &[DependencyEnvironmentEntry]) -> bool + 'a;

/// The artifact snapshot root of the files installed for the package the
/// specifier names (`installed_package_snapshot_root`), or why this tree
/// cannot state that its files are the published archive's: a patch the tree
/// records, a member no archive installs, an unreadable file.
///
/// Step 2's acceptance root reproduces from the lockfile's integrity, which a
/// package manager keeps when it patches the installed files, so the files
/// themselves are asked too (ADR 0131). The native answer walks the installed
/// directory; a host with no filesystem has no answer and admits nothing that
/// needs one.
/// The second argument admits a patched install (ADR 0208): only an authored
/// entry that states its snapshot root is of one asks it.
pub type InstalledArtifactBytes<'a> = dyn Fn(&str, bool) -> Result<String, String> + 'a;

/// Whether an installed tree reproduces `environment`, starting from `root`,
/// the imported package's installed location.
///
/// `resolve(from, name)` is Node's lookup of the bare package `name` from the
/// package installed at `from`: `Ok(None)` when nothing is installed under any
/// `node_modules` it walks, `Err` when the tree cannot state the answer
/// exactly. `identity(at)` is the name, manifest version and lockfile integrity
/// of the package installed at `at`, or `None` when those are not all stated.
///
/// **An environment that states its resolution edges** (the `edges:v3` root)
/// is replayed edge by edge. Each entry names the package that looked it up and
/// the bare name it looked up; the lookup is repeated from that importer's own
/// installed location -- `root` for the certified package, and for any other
/// importer every location an earlier edge reached it at -- and must reach
/// exactly that entry's name, manifest version and lockfile integrity.
///
/// - What the certification never looked up is not asked. A package elsewhere
///   in the tree that sees another, hoisted version of the same name is not a
///   premise of the proof: in a pnpm tree `.pnpm/node_modules` hoists some
///   version of nearly everything into every package's view, and a rule that
///   read those lookups would refuse the tree the certification ran in.
/// - One name may appear under two importers at two versions, because each
///   edge is its own lookup.
/// - Missing, unresolvable or different refuses, as does an entry whose
///   importer is never reached.
///
/// **An environment stated without edges** (the `v1` root, which every receipt
/// issued before edges were recorded binds) cannot say which package read each
/// entry, so it keeps the strict rule, which is sound for it:
///
/// - Every entry must be found from at least one located package: the root, or
///   a package an earlier entry resolved to. An entry nothing reaches is a
///   premise this tree cannot supply.
/// - **Every** resolution of an entry's name, from **every** located package,
///   must reach exactly that entry's identity. A nested copy under one
///   dependency that differs from the hoisted one another dependency sees is
///   exactly the swap this cannot tell apart from the certified tree. It is
///   refused rather than guessed.
/// - Two entries with the same name -- two copies of one package in the
///   certified environment -- can therefore never both hold, and refuse.
///
/// **Under either rule, a copy that matches but is patched refuses** (ADR
/// 0131). `patched(at)` names the evidence that the package installed at `at`
/// is not the published archive its lockfile integrity names -- a package
/// manager's patch record, a `patch-package` patch -- or `None` when the tree
/// shows none. A lockfile keeps the published integrity for a patched package,
/// so name, version and integrity alone would admit a contract about bytes the
/// consumer does not run.
pub(crate) fn environment_is_installed<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
) -> bool {
    environment_difference(environment, root, resolve, identity, patched, |_| None).is_none()
}

/// The first way an installed tree fails to reproduce `environment`, by the
/// rule [`environment_is_installed`] states; `None` when it reproduces it.
///
/// `version_of(at)` is the manifest version of the package at `at` when it can
/// be read even though its full identity cannot, so a report can name the
/// installed version beside the certified one. It never decides anything.
pub(crate) fn environment_difference<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
    version_of: impl Fn(&L) -> Option<String>,
) -> Option<EnvironmentDifference> {
    if dependency_environment_states_edges(environment) {
        return edge_environment_difference(
            environment,
            root,
            resolve,
            identity,
            patched,
            version_of,
        );
    }
    let mut names = BTreeSet::new();
    if let Some(entry) = environment
        .iter()
        .find(|entry| !names.insert(entry.name.as_str()))
    {
        return Some(EnvironmentDifference::DuplicateName {
            name: entry.name.clone(),
        });
    }
    let mut located = BTreeSet::from([root.clone()]);
    let mut pending = vec![root];
    let mut found = BTreeSet::new();
    while let Some(from) = pending.pop() {
        for entry in environment {
            let at = match resolve(&from, &entry.name) {
                Ok(Some(at)) => at,
                Ok(None) => continue,
                Err(()) => {
                    return Some(EnvironmentDifference::Unresolvable {
                        name: entry.name.clone(),
                        from: None,
                    });
                }
            };
            let installed = identity(&at);
            if !installed
                .as_ref()
                .is_some_and(|installed| installed.same_package(entry))
            {
                return Some(EnvironmentDifference::Differs {
                    installed_version: installed
                        .as_ref()
                        .map(|installed| installed.version.clone())
                        .or_else(|| version_of(&at)),
                    installed,
                    certified: entry.clone(),
                    from: None,
                });
            }
            if let Some(evidence) = patched(&at) {
                return Some(EnvironmentDifference::Patched {
                    certified: entry.clone(),
                    from: None,
                    evidence,
                });
            }
            found.insert(entry.name.as_str());
            if located.insert(at.clone()) {
                pending.push(at);
            }
        }
    }
    environment
        .iter()
        .find(|entry| !found.contains(entry.name.as_str()))
        .map(|entry| EnvironmentDifference::NotInstalled {
            certified: entry.clone(),
            from: None,
        })
}

/// The edge-rooted rule [`environment_is_installed`] states for an environment
/// that records who resolved what. Breadth-first from the certified package,
/// so the first failing edge reported is the one nearest the root.
fn edge_environment_difference<L: Clone + Ord>(
    environment: &[DependencyEnvironmentEntry],
    root: L,
    resolve: impl Fn(&L, &str) -> Result<Option<L>, ()>,
    identity: impl Fn(&L) -> Option<DependencyEnvironmentEntry>,
    patched: impl Fn(&L) -> Option<String>,
    version_of: impl Fn(&L) -> Option<String>,
) -> Option<EnvironmentDifference> {
    let certified_label = identity(&root).map_or_else(
        || "the certified package".to_owned(),
        |certified| format!("{}@{}", certified.name, certified.version),
    );
    let label = |importer: &EnvironmentImporter| match importer {
        EnvironmentImporter::Certified => certified_label.clone(),
        EnvironmentImporter::Package(package) => format!("{}@{}", package.name, package.version),
    };
    let mut located = BTreeMap::<EnvironmentImporter, BTreeSet<L>>::new();
    located
        .entry(EnvironmentImporter::Certified)
        .or_default()
        .insert(root.clone());
    // Each importer's edges, in canonical order, so every location is asked
    // only the lookups made from it.
    let mut by_importer = BTreeMap::<&EnvironmentImporter, Vec<usize>>::new();
    for (index, entry) in environment.iter().enumerate() {
        if let Some(edge) = &entry.resolved_from {
            by_importer.entry(&edge.importer).or_default().push(index);
        }
    }
    let mut queue = VecDeque::from([(EnvironmentImporter::Certified, root)]);
    let mut reached = vec![false; environment.len()];
    while let Some((importer, from)) = queue.pop_front() {
        for &index in by_importer.get(&importer).map_or(&[][..], Vec::as_slice) {
            let entry = &environment[index];
            let Some(edge) = &entry.resolved_from else {
                continue;
            };
            let at = match resolve(&from, &edge.specifier) {
                Ok(Some(at)) => at,
                Ok(None) => {
                    return Some(EnvironmentDifference::NotInstalled {
                        certified: entry.clone(),
                        from: Some(label(&importer)),
                    });
                }
                Err(()) => {
                    return Some(EnvironmentDifference::Unresolvable {
                        name: edge.specifier.clone(),
                        from: Some(label(&importer)),
                    });
                }
            };
            let installed = identity(&at);
            if !installed
                .as_ref()
                .is_some_and(|installed| installed.same_package(entry))
            {
                return Some(EnvironmentDifference::Differs {
                    installed_version: installed
                        .as_ref()
                        .map(|installed| installed.version.clone())
                        .or_else(|| version_of(&at)),
                    installed,
                    certified: entry.clone(),
                    from: Some(label(&importer)),
                });
            }
            if let Some(evidence) = patched(&at) {
                return Some(EnvironmentDifference::Patched {
                    certified: entry.clone(),
                    from: Some(label(&importer)),
                    evidence,
                });
            }
            reached[index] = true;
            let key = entry.as_importer();
            if located.entry(key.clone()).or_default().insert(at.clone()) {
                queue.push_back((key, at));
            }
        }
    }
    // Validation refuses an environment whose edges are not rooted, so this is
    // reached only for an entry whose importer this tree never located.
    environment
        .iter()
        .zip(reached)
        .find(|(_, reached)| !reached)
        .map(|(entry, _)| EnvironmentDifference::NotInstalled {
            certified: entry.clone(),
            from: entry
                .resolved_from
                .as_ref()
                .map(|edge| label(&edge.importer)),
        })
}

/// Why an installed tree does not reproduce a certified environment: the
/// first entry that differs, for a report that has to name it.
///
/// `from` names the importer whose lookup failed (`name@version`) when the
/// environment states edges; `None` under the strict rule, which has no edge
/// to name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EnvironmentDifference {
    /// The certified environment names one package twice, which no tree can
    /// reproduce under the "every resolution" rule.
    DuplicateName { name: String },
    /// The tree cannot state exactly which copy of this package resolves.
    Unresolvable { name: String, from: Option<String> },
    /// A copy resolves, and it is not the certified one.
    Differs {
        /// Its full identity, when the tree states one.
        installed: Option<DependencyEnvironmentEntry>,
        /// Its manifest version, when readable.
        installed_version: Option<String>,
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
    },
    /// Nothing resolves this certified package from anywhere the rule looks.
    NotInstalled {
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
    },
    /// The certified copy resolves -- name, version and lockfile integrity all
    /// match -- and the tree shows it patched, so its bytes are not the
    /// published archive that integrity names (ADR 0131).
    Patched {
        certified: DependencyEnvironmentEntry,
        from: Option<String>,
        /// What shows the patch, e.g. `pnpm-lock.yaml patchedDependencies`.
        evidence: String,
    },
}

impl std::fmt::Display for EnvironmentDifference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let via = |from: &Option<String>| {
            from.as_ref()
                .map_or_else(String::new, |from| format!(" resolved from {from}"))
        };
        match self {
            Self::DuplicateName { name } => {
                write!(formatter, "the certified environment names {name} twice")
            }
            Self::Unresolvable { name, from } => write!(
                formatter,
                "{name}{}: this tree cannot state exactly which installed copy resolves",
                via(from)
            ),
            Self::Differs {
                installed: Some(installed),
                certified,
                from,
                ..
            } if installed.version == certified.version => write!(
                formatter,
                "{}{} {} installed with integrity {}, certified with {}",
                certified.name,
                via(from),
                certified.version,
                installed.integrity,
                certified.integrity
            ),
            Self::Differs {
                installed_version: Some(version),
                certified,
                from,
                ..
            } => write!(
                formatter,
                "{}{} installed {version}, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::Differs {
                certified, from, ..
            } => write!(
                formatter,
                "{}{} installed with no exact version and lockfile integrity, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::NotInstalled { certified, from } => write!(
                formatter,
                "{}{} not installed, certified {}",
                certified.name,
                via(from),
                certified.version
            ),
            Self::Patched {
                certified,
                from,
                evidence,
            } => write!(
                formatter,
                "{}@{}{} is patched ({evidence}), so its installed bytes are not the published \
                 archive the certification read",
                certified.name,
                certified.version,
                via(from)
            ),
        }
    }
}

/// Why [`admit_by_artifact`] did not admit an acceptance, in the order its
/// steps are checked. A report states it so `missing` is not the whole answer
/// when an acceptance for the installed package does exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    /// Step 1: the receipt states no dependency environment -- issued before
    /// ADR 0123, by a certifier that could not acquire one, or binding the
    /// retired ambiguous empty root.
    NoEnvironmentStated,
    /// Step 2: this project's installed identity does not reproduce the signed
    /// acceptance root. `installed` is `(version, integrity)` when the project
    /// states them; `None` when it cannot (no exact lockfile integrity).
    AcceptanceRootNotReproduced {
        installed: Option<(String, String)>,
        certified_version: String,
    },
    /// Step 2b: the installed package's files are not the archive the receipt
    /// signs (`snapshotRoot`) -- patched, rebuilt or edited after install; the
    /// text says what shows it (ADR 0131).
    InstalledBytesDiffer(String),
    /// Step 3: the installed tree differs from the certified environment; the
    /// text names the first differing package.
    EnvironmentDiffers(String),
    /// Step 1b (ADR 0151): the receipt cites a compiled-in acceptance, and
    /// the compiled-in tier is retired (ADR 0228), so the claim it was built
    /// on has been withdrawn; the text names the citation.
    CitationWithdrawn(String),
}

impl std::fmt::Display for AdmissionRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEnvironmentStated => formatter.write_str(
                "its receipt states no dependency environment, so it is admitted nowhere",
            ),
            Self::AcceptanceRootNotReproduced {
                installed: Some((version, _)),
                certified_version,
            } if version != certified_version => write!(
                formatter,
                "the installed package is {version}, not the certified {certified_version}"
            ),
            Self::AcceptanceRootNotReproduced {
                installed: Some(_), ..
            } => formatter.write_str(
                "the installed package's bytes or requested entrypoint are not the certified ones \
                 (its acceptance root is not reproduced)",
            ),
            Self::AcceptanceRootNotReproduced {
                installed: None, ..
            } => formatter.write_str(
                "the installed package has no exact lockfile integrity, so its acceptance root \
                 cannot be reproduced",
            ),
            Self::InstalledBytesDiffer(difference) => write!(
                formatter,
                "the installed package's files are not the certified archive's: {difference}"
            ),
            Self::EnvironmentDiffers(difference) => {
                write!(
                    formatter,
                    "its dependency environment differs: {difference}"
                )
            }
            Self::CitationWithdrawn(citation) => write!(
                formatter,
                "it cites the compiled-in acceptance {citation}, and the compiled-in tier \
                 is retired"
            ),
        }
    }
}

/// The first way this tree differs from a certified environment, rendered, or
/// `None` when it reproduces it. The diagnostic twin of
/// [`InstalledEnvironment`]; it never decides admission.
pub type InstalledEnvironmentDifference<'a> =
    dyn Fn(&str, &[DependencyEnvironmentEntry]) -> Option<String> + 'a;

/// Steps 1-3 of [`admit_by_artifact`] for each acceptance, reporting the first
/// that fails: `Some(refusal)`, or `None` when the acceptance passes all three
/// and admission then turns only on the resolved file and case selection.
///
/// Diagnostic only, and deliberately a replay of the same checks rather than a
/// second rule: nothing admits from it.
pub(crate) fn admission_refusals<'a>(
    acceptances: impl IntoIterator<Item = (ArtifactAcceptance<'a>, &'a str)>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    installed_difference: &InstalledEnvironmentDifference,
) -> Vec<(String, Option<AdmissionRefusal>)> {
    acceptances
        .into_iter()
        .map(|(acceptance, certified_version)| {
            let refusal = (|| {
                let Some(environment) = acceptance.environment else {
                    return Some(AdmissionRefusal::NoEnvironmentStated);
                };
                if let Some(withdrawn) = withdrawn_compiled_in_citation(acceptance.citations) {
                    return Some(AdmissionRefusal::CitationWithdrawn(withdrawn));
                }
                let Some((name, version, integrity)) = installed_integrity(acceptance.specifier)
                else {
                    return Some(AdmissionRefusal::AcceptanceRootNotReproduced {
                        installed: None,
                        certified_version: certified_version.to_owned(),
                    });
                };
                if policy2_artifact_acceptance_root_for_identity(
                    &name,
                    &version,
                    &integrity,
                    acceptance.requested_entrypoint,
                    acceptance.export_conditions,
                ) != acceptance.acceptance_root
                {
                    return Some(AdmissionRefusal::AcceptanceRootNotReproduced {
                        installed: Some((version, integrity)),
                        certified_version: certified_version.to_owned(),
                    });
                }
                if let Some(difference) = bytes_difference(&acceptance, installed_bytes) {
                    return Some(AdmissionRefusal::InstalledBytesDiffer(difference));
                }
                installed_difference(acceptance.specifier, environment)
                    .map(AdmissionRefusal::EnvironmentDiffers)
            })();
            (acceptance.specifier.to_owned(), refusal)
        })
        .collect()
}

/// Why step 2b refuses `acceptance`, or `None` when the installed files
/// reproduce the signed snapshot root. The one reading of that step, for both
/// admission and the refusal report.
fn bytes_difference(
    acceptance: &ArtifactAcceptance,
    installed_bytes: &InstalledArtifactBytes,
) -> Option<String> {
    if acceptance.snapshot_root.is_empty() {
        return Some("its receipt signs no snapshot root to compare them with".into());
    }
    match installed_bytes(acceptance.specifier, acceptance.patched_install) {
        Ok(root) if root == acceptance.snapshot_root => None,
        Ok(_) => Some(
            "they do not reproduce the signed snapshot root, so something changed them after \
             install"
                .into(),
        ),
        Err(reason) => Some(reason),
    }
}

/// One acceptance a consumer could reach by artifact rather than by importer,
/// from whichever supply it came from, described by exactly what the consumer's
/// own installed tree can be compared against.
pub(crate) struct ArtifactAcceptance<'a> {
    /// The specifier a consumer writes; admission is keyed by it.
    pub(crate) specifier: &'a str,
    pub(crate) requested_entrypoint: &'a str,
    /// The conditions the signed acceptance root was computed over.
    pub(crate) export_conditions: &'a [String],
    /// Package-relative; see [`AuthenticCase::reaches`].
    pub(crate) runtime_target: &'a str,
    pub(crate) declaration_target: &'a str,
    /// The signed `artifactAcceptanceRoot`.
    pub(crate) acceptance_root: &'a str,
    /// The signed `snapshotRoot`, which the installed files must reproduce.
    pub(crate) snapshot_root: &'a str,
    /// ADR 0208: the snapshot root is of a patched install, so a patch the
    /// tree records does not refuse the comparison. Only an authored entry
    /// states it; every certified acceptance is about the published archive.
    pub(crate) patched_install: bool,
    /// The entries behind the signed `dependencyEnvironmentRoot`, already
    /// reproduced against it. `None` when the receipt states no environment
    /// or its entries were not published: such an acceptance is never admitted.
    pub(crate) environment: Option<&'a [DependencyEnvironmentEntry]>,
    /// The key the acceptance is indexed under
    /// ([`environment_acceptance_identity`]).
    pub(crate) identity: &'a str,
    /// The compiled-in acceptances its receipt cites (ADR 0151). The
    /// compiled-in tier is retired (ADR 0228), so any citation refuses.
    pub(crate) citations: &'a [CitedAcceptance],
}

/// The one rule for admitting an acceptance by artifact (ADR 0123), whichever
/// supply it came from.
///
/// An acceptance applies to this project only when all of these hold:
///
/// 1. the receipt states the environment its proof read;
///    1b. its receipt cites no compiled-in acceptance (ADR 0151): that tier is
///    retired (ADR 0228);
/// 2. the project's installed identity for the specifier -- name, manifest
///    version, lockfile integrity -- reproduces the signed acceptance root under
///    the conditions it was computed over;
///    2b. the files installed for that package reproduce the signed
///    `snapshotRoot`, so they are the archive the proof read and not a patched
///    copy that kept its lockfile integrity (ADR 0131);
/// 3. the project's installed tree, resolved from that package's own location,
///    reproduces the environment exactly, and no package in it is patched
///    ([`environment_is_installed`]);
/// 4. the file this project resolved is one the acceptance was proven about,
///    and [`admissible_cases`] selects it under the host's declaration.
///
/// Anything missing, different or unstatable skips the acceptance, so this
/// adds acceptances on proof and never removes one.
///
pub(crate) fn admit_by_artifact<'a>(
    acceptances: impl IntoIterator<Item = ArtifactAcceptance<'a>>,
    conditions: &BTreeSet<String>,
    installed_integrity: &InstalledArtifactIdentity,
    installed_bytes: &InstalledArtifactBytes,
    resolved_target: &ResolvedTargetIdentity,
    installed_environment: &InstalledEnvironment,
) -> Vec<(String, String)> {
    let declared = declared_conditions(conditions);
    let mut authentic: BTreeMap<String, Vec<AuthenticCase>> = BTreeMap::new();
    for acceptance in acceptances {
        let Some(environment) = acceptance.environment else {
            continue;
        };
        // ADR 0151, ADR 0228: built on a compiled-in claim no build carries.
        if withdrawn_compiled_in_citation(acceptance.citations).is_some() {
            continue;
        }
        let Some((name, version, integrity)) = installed_integrity(acceptance.specifier) else {
            continue;
        };
        // Recomputed against the *installed* identity rather than the
        // certifier's record of it: the record states what certification
        // resolved, and the question here is whether this project resolved the
        // same thing.
        let derived = policy2_artifact_acceptance_root_for_identity(
            &name,
            &version,
            &integrity,
            acceptance.requested_entrypoint,
            acceptance.export_conditions,
        );
        if derived != acceptance.acceptance_root {
            continue;
        }
        // The integrity is the lockfile's record of what was fetched, not of
        // what is on disk: a patched package keeps the published one. The
        // files themselves have to be the archive the proof read.
        if bytes_difference(&acceptance, installed_bytes).is_some() {
            continue;
        }
        // The same artifact, and now the same environment, or it is a
        // different acceptance this project never reproduced.
        if !installed_environment(acceptance.specifier, environment) {
            continue;
        }
        authentic
            .entry(acceptance.specifier.to_owned())
            .or_default()
            .push(AuthenticCase::from_relative(
                acceptance.identity.to_owned(),
                acceptance.runtime_target.to_owned(),
                acceptance.declaration_target.to_owned(),
                acceptance.export_conditions.to_vec(),
                environment.len(),
            ));
    }
    let mut admitted = Vec::new();
    for (specifier, cases) in authentic {
        // What this project resolved the specifier to. Without it nothing is
        // admitted.
        let Some(target) = resolved_target(&specifier) else {
            continue;
        };
        // TypeScript resolves the *declaration* file, and one `.d.ts` is
        // routinely shared by several export-condition branches. So the
        // resolved file selects a set of candidate cases, not one case.
        let reaching = cases
            .iter()
            .filter(|case| case.reaches(&target))
            .collect::<Vec<_>>();
        admitted.extend(
            admissible_cases(&reaching, &declared)
                .into_iter()
                .map(|case| (specifier.clone(), case.identity.clone())),
        );
    }
    admitted
}
