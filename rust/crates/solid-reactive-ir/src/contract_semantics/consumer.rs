//! Demand-sensitive analyzer queries over accepted normalized semantics.
//!
//! This module is deliberately the only adapter from exact call-site facts to
//! guard selection. Consumers receive local knowledge plus local refusal
//! reasons; they never inspect compact-wire closure or summary mechanics.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AcceptedSemanticIdentity {
    pub package: PackageIdentity,
    pub artifact_case: String,
    pub receipt_version: u16,
    pub semantic_model_version: u16,
    pub semantic_digest: Digest,
    pub artifacts_digest: Digest,
    pub closure_digest: Digest,
    pub proof_root: Digest,
    pub closed_claims_root: Digest,
    pub verifier: VerifierIdentity,
    pub authentication: Option<ReceiptAuthenticationIdentity>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AcceptedImportIdentity {
    pub importer: String,
    pub specifier: String,
    pub semantics: AcceptedSemanticIdentity,
}

/// One already-loaded contract at the exact import occurrence whose resolver
/// selected its artifact case. The importer is part of identity because two
/// nested installations may resolve the same specifier differently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContractInput {
    pub importer: String,
    pub specifier: String,
    pub contract: AcceptedContract,
    /// The receipt's importer-free artifact identity, when the loader could
    /// establish it. `None` keeps this acceptance importer-only: it is the
    /// fail-closed direction, and the loader uses it for anything it cannot
    /// state exactly — see `artifact_identity` below.
    pub artifact_identity: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UncertifiableImportReason {
    Unspecified,
    ObsoletePolicy1,
}

/// A resolved analyzer use, retaining exact identity rather than a public name
/// lookup that could later be rebound by a reexport.
#[derive(Debug)]
pub struct AcceptedContractUse<'a> {
    contract: &'a AcceptedContract,
    identity: ExportIdentity,
}

impl AcceptedContractUse<'_> {
    #[must_use]
    pub const fn contract(&self) -> &AcceptedContract {
        self.contract
    }

    #[must_use]
    pub const fn identity(&self) -> &ExportIdentity {
        &self.identity
    }

    #[must_use]
    pub fn export(&self) -> &ExportSemantics {
        self.contract
            .export(&self.identity.public_name)
            .expect("accepted use was resolved from this export")
    }

    pub fn instantiate<'contract, 'facts>(
        &'contract self,
        facts: &'facts CallSiteFacts,
    ) -> Result<InstantiatedExport<'contract, 'facts>, SemanticQueryError> {
        self.contract.instantiate_export(&self.identity, facts)
    }
}

/// Analyzer-facing import index. Acquisition and receipt validation happen
/// before construction; consumers ask only for one exact import/export use.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedContractIndex {
    imports: BTreeMap<(String, String), Vec<AcceptedContract>>,
    /// Acceptances reachable by the artifact they were proven about, rather
    /// than by the file that imported it during certification. An entry here
    /// is an addition to `imports`, never a replacement: a consumer that
    /// matches by importer is answered exactly as before.
    by_artifact: BTreeMap<String, Vec<AcceptedContract>>,
    /// Specifiers whose installed artifact in *this* project is one an
    /// acceptance was issued for, so any file may import them. Populated only
    /// by `with_admitted_artifacts`; empty otherwise, which is every path that
    /// does not derive identities.
    admitted: BTreeMap<String, AcceptedContract>,
    uncertifiable_imports: BTreeMap<(String, String), UncertifiableImportReason>,
    identity: Vec<AcceptedImportIdentity>,
}

impl AcceptedContractIndex {
    /// Ordinary analysis has one authority for Solid core: the built-in
    /// dialect. Independent certification may still retain core contracts in
    /// this general index, but they cannot supplement the runtime model.
    /// Filter by authenticated package identity as well as written specifier,
    /// so an alias cannot introduce a second authority. This is withholding,
    /// never evidence that the specifier actually resolves to Solid.
    #[must_use]
    pub fn external_packages(&self) -> std::borrow::Cow<'_, Self> {
        fn core_specifier(specifier: &str) -> bool {
            solid_dialect::core_runtime_contract_reference("", specifier)
        }

        let retain = |key: &(String, String), contracts: &[AcceptedContract]| {
            !core_specifier(&key.1)
                && contracts.iter().all(|contract| {
                    !solid_dialect::primitive_defining_package(&contract.package().name)
                })
        };
        if self.imports.iter().all(|(key, values)| retain(key, values))
            && self
                .uncertifiable_imports
                .keys()
                .all(|(_, specifier)| !core_specifier(specifier))
        {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut external = self.clone();
        external.imports.retain(|key, values| retain(key, values));
        let external_package = |contract: &AcceptedContract| {
            !solid_dialect::primitive_defining_package(&contract.package().name)
        };
        external
            .by_artifact
            .retain(|_, values| values.iter().all(external_package));
        external
            .admitted
            .retain(|specifier, contract| !core_specifier(specifier) && external_package(contract));
        external.identity.retain(|identity| {
            external
                .imports
                .contains_key(&(identity.importer.clone(), identity.specifier.clone()))
        });
        external
            .uncertifiable_imports
            .retain(|(_, specifier), _| !core_specifier(specifier));
        std::borrow::Cow::Owned(external)
    }

    /// Acceptances a project never imported by name: each one is reachable
    /// only through the artifact it was proven about.
    ///
    /// This is the compiled-in tier's shape. A bundle has no importer in this
    /// project — the file that imported it during certification is on another
    /// machine — so putting it in `imports` would key it on a path that cannot
    /// occur here, and would make every report that enumerates
    /// `semantic_identity` claim the project accepted a contract it never
    /// reached. Artifact admission is the whole match, exactly as it is for a
    /// catalog entry whose importer does not match either.
    #[must_use]
    pub fn from_artifact_acceptances(
        inputs: impl IntoIterator<Item = (String, AcceptedContract)>,
    ) -> Self {
        let mut by_artifact = BTreeMap::<String, Vec<AcceptedContract>>::new();
        for (identity, contract) in inputs {
            by_artifact.entry(identity).or_default().push(contract);
        }
        by_artifact.retain(|_, contracts| {
            contracts.len() == 1
                || contracts
                    .windows(2)
                    .all(|pair| pair[0].semantic_identity() == pair[1].semantic_identity())
        });
        Self {
            imports: BTreeMap::new(),
            by_artifact,
            admitted: BTreeMap::new(),
            uncertifiable_imports: BTreeMap::new(),
            identity: Vec::new(),
        }
    }

    pub fn new(
        inputs: impl IntoIterator<Item = AcceptedContractInput>,
    ) -> Result<Self, SemanticQueryError> {
        let mut imports = BTreeMap::<_, Vec<_>>::new();
        let mut by_artifact = BTreeMap::<String, Vec<AcceptedContract>>::new();
        for input in inputs {
            if let Some(identity) = input.artifact_identity {
                by_artifact
                    .entry(identity)
                    .or_default()
                    .push(input.contract.clone());
            }
            imports
                .entry((input.importer, input.specifier))
                .or_default()
                .push(input.contract);
        }
        // An artifact identity naming two different contracts is not a
        // preference to resolve: it is two answers about the same bytes, and
        // neither may be applied. Dropping the entry leaves those acceptances
        // importer-only rather than failing the catalog, because the
        // importer-keyed answers are still exactly as sound as they were.
        by_artifact.retain(|_, contracts| {
            contracts.len() == 1
                || contracts
                    .windows(2)
                    .all(|pair| pair[0].semantic_identity() == pair[1].semantic_identity())
        });
        let mut identity = Vec::new();
        for ((importer, specifier), contracts) in &imports {
            if contracts.len() != 1 {
                return Err(SemanticQueryError::AmbiguousImport {
                    importer: importer.clone(),
                    specifier: specifier.clone(),
                });
            }
            identity.push(AcceptedImportIdentity {
                importer: importer.clone(),
                specifier: specifier.clone(),
                semantics: contracts[0].semantic_identity(),
            });
        }
        identity.sort();
        Ok(Self {
            imports,
            by_artifact,
            admitted: BTreeMap::new(),
            uncertifiable_imports: BTreeMap::new(),
            identity,
        })
    }

    #[must_use]
    pub fn with_uncertifiable_imports(
        mut self,
        imports: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.uncertifiable_imports.extend(
            imports
                .into_iter()
                .map(|key| (key, UncertifiableImportReason::Unspecified)),
        );
        self.uncertifiable_imports
            .retain(|key, _| !self.imports.contains_key(key));
        self
    }

    #[must_use]
    pub fn is_uncertifiable(&self, importer: &str, specifier: &str) -> bool {
        self.uncertifiable_imports
            .contains_key(&(importer.to_owned(), specifier.to_owned()))
    }

    #[must_use]
    pub fn uncertifiable_reason(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Option<UncertifiableImportReason> {
        self.uncertifiable_imports
            .get(&(importer.to_owned(), specifier.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn with_uncertifiable_import_reasons(
        mut self,
        imports: impl IntoIterator<Item = ((String, String), UncertifiableImportReason)>,
    ) -> Self {
        self.uncertifiable_imports.extend(imports);
        self.uncertifiable_imports
            .retain(|key, _| !self.imports.contains_key(key));
        self
    }

    /// Adds fallback accepted imports without replacing an exact host entry.
    /// This is the only supported composition rule for project catalogs and
    /// receipt-issued built-ins: project acquisition wins per importer and
    /// specifier, while a built-in may fill only a genuinely absent key.
    pub fn with_fallback(mut self, fallback: Self) -> Self {
        for (key, contracts) in fallback.imports {
            self.imports.entry(key).or_insert(contracts);
        }
        // The artifact index is unioned for the same reason the import index
        // is. Leaving it out made folding several catalogs keep only the last
        // one's artifacts, and `with_admitted_artifacts` then silently found
        // nothing for every other catalog's acceptance -- which is what a
        // project with two certified dependencies has, because `contract
        // certify` publishes a plain catalog for a single-case package and a
        // case set for a multi-case one. Measured: two certified packages, both
        // reported `missing`.
        for (identity, contracts) in fallback.by_artifact {
            self.by_artifact.entry(identity).or_insert(contracts);
        }
        self.identity = self
            .imports
            .iter()
            .filter_map(|((importer, specifier), contracts)| {
                let [contract] = contracts.as_slice() else {
                    return None;
                };
                Some(AcceptedImportIdentity {
                    importer: importer.clone(),
                    specifier: specifier.clone(),
                    semantics: contract.semantic_identity(),
                })
            })
            .collect();
        self.identity.sort();
        self.uncertifiable_imports
            .extend(fallback.uncertifiable_imports);
        self.uncertifiable_imports
            .retain(|key, _| !self.imports.contains_key(key));
        self
    }

    #[must_use]
    pub fn semantic_identity(&self) -> &[AcceptedImportIdentity] {
        &self.identity
    }

    /// Canonical cache key for every exact import binding and every receipt
    /// component that authorizes analyzer-visible meaning.
    #[must_use]
    pub fn cache_fingerprint(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"solid-checker-accepted-contract-index-v3");
        hash.update((self.identity.len() as u64).to_be_bytes());
        for binding in &self.identity {
            hash_text(&mut hash, &binding.importer);
            hash_text(&mut hash, &binding.specifier);
            let semantic = &binding.semantics;
            hash_text(&mut hash, &semantic.package.name);
            hash_text(&mut hash, &semantic.package.version);
            hash_text(&mut hash, &semantic.package.integrity);
            hash_text(&mut hash, &semantic.package.manifest.path);
            hash_text(&mut hash, semantic.package.manifest.digest.as_str());
            hash_text(&mut hash, &semantic.artifact_case);
            hash.update(semantic.receipt_version.to_be_bytes());
            hash.update(semantic.semantic_model_version.to_be_bytes());
            hash_text(&mut hash, semantic.semantic_digest.as_str());
            hash_text(&mut hash, semantic.artifacts_digest.as_str());
            hash_text(&mut hash, semantic.closure_digest.as_str());
            hash_text(&mut hash, semantic.proof_root.as_str());
            hash_text(&mut hash, semantic.closed_claims_root.as_str());
            hash_text(&mut hash, &semantic.verifier.build);
            hash.update(semantic.verifier.policy.to_be_bytes());
            match &semantic.authentication {
                Some(authentication) => {
                    hash.update([1]);
                    hash_text(&mut hash, authentication.receipt_digest.as_str());
                    hash_text(&mut hash, authentication.policy_digest.as_str());
                    hash_text(&mut hash, authentication.trust_store_digest.as_str());
                    hash.update(authentication.revocation_epoch.to_be_bytes());
                }
                None => hash.update([0]),
            }
        }
        hash.update((self.admitted.len() as u64).to_be_bytes());
        for (specifier, contract) in &self.admitted {
            hash_text(&mut hash, specifier);
            let semantic = contract.semantic_identity();
            hash_text(&mut hash, &semantic.package.name);
            hash_text(&mut hash, &semantic.package.version);
            hash_text(&mut hash, &semantic.package.integrity);
            hash_text(&mut hash, &semantic.artifact_case);
            hash_text(&mut hash, semantic.semantic_digest.as_str());
            hash_text(&mut hash, semantic.closed_claims_root.as_str());
            match &semantic.authentication {
                Some(authentication) => {
                    hash.update([1]);
                    hash_text(&mut hash, authentication.receipt_digest.as_str());
                    hash_text(&mut hash, authentication.trust_store_digest.as_str());
                }
                None => hash.update([0]),
            }
        }
        hash.update((self.uncertifiable_imports.len() as u64).to_be_bytes());
        for ((importer, specifier), reason) in &self.uncertifiable_imports {
            hash_text(&mut hash, importer);
            hash_text(&mut hash, specifier);
            hash.update([match reason {
                UncertifiableImportReason::Unspecified => 0,
                UncertifiableImportReason::ObsoletePolicy1 => 1,
            }]);
        }
        hash.finalize().into()
    }

    pub fn resolve<'a>(
        &'a self,
        importer: &str,
        specifier: &str,
        identity: &ExportIdentity,
    ) -> Result<AcceptedContractUse<'a>, SemanticQueryError> {
        let contract = self
            .imports
            .get(&(importer.to_owned(), specifier.to_owned()))
            .and_then(|contracts| contracts.first())
            .ok_or_else(|| SemanticQueryError::MissingImport {
                importer: importer.into(),
                specifier: specifier.into(),
            })?;
        contract.resolve_export(identity)?;
        Ok(AcceptedContractUse {
            contract,
            identity: identity.clone(),
        })
    }

    /// Resolves one public spelling only after the exact importer/specifier
    /// pair has already selected a single receipt-validated artifact case.
    /// The returned use retains the full runtime/declaration export identity;
    /// this is not package-name or export-name-only contract selection.
    pub fn resolve_name<'a>(
        &'a self,
        importer: &str,
        specifier: &str,
        public_name: &str,
    ) -> Result<AcceptedContractUse<'a>, SemanticQueryError> {
        let contract = self.contract(importer, specifier)?;
        let export =
            contract
                .export(public_name)
                .ok_or_else(|| SemanticQueryError::MissingExport {
                    export: public_name.into(),
                })?;
        contract.resolve_export(&export.identity)?;
        Ok(AcceptedContractUse {
            contract,
            identity: export.identity.clone(),
        })
    }

    pub fn contract(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Result<&AcceptedContract, SemanticQueryError> {
        self.imports
            .get(&(importer.to_owned(), specifier.to_owned()))
            .and_then(|contracts| contracts.first())
            .or_else(|| self.admitted.get(specifier))
            .ok_or_else(|| SemanticQueryError::MissingImport {
                importer: importer.into(),
                specifier: specifier.into(),
            })
    }

    /// Finds the acceptance issued for exactly this artifact, whatever file
    /// imported it when the contract was certified.
    ///
    /// The caller supplies an identity it derived from its *own* resolution,
    /// and equality of that identity is the whole check: it commits to the
    /// package's tarball integrity, its entrypoint and the export conditions,
    /// so an equal identity is the same published bytes reached the same way.
    /// The importer is deliberately not consulted — it is what this lookup
    /// exists to stop requiring — and an identity the loader could not state
    /// exactly is simply absent here.
    /// Admits a specifier project-wide when this project's *installed* artifact
    /// is the one an accepted contract was proven about.
    ///
    /// Each entry is `(specifier, artifact identity)` derived by the caller from
    /// the installed tree: the package's registry integrity, its entrypoint and
    /// the host's declared export conditions. An identity that matches no
    /// acceptance is skipped.
    ///
    /// This is where an acceptance stops being bound to the file that imported
    /// it during certification. What justifies dropping the importer is that the
    /// identity commits to the tarball integrity: if the installed bytes, the
    /// entrypoint and the conditions are the same, every importer in this
    /// project reaches the artifact the contract was proven about, whatever file
    /// it was certified from. A nested install with different bytes derives a
    /// different identity and is not admitted — the caller must refuse to state
    /// an identity when the project's installs disagree, rather than pick one.
    ///
    /// Importer-keyed acceptances still win: `contract` consults them first, so
    /// a catalog entry naming an exact file is never displaced by this.
    #[must_use]
    pub fn with_admitted_artifacts(
        mut self,
        admitted: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        for (specifier, identity) in admitted {
            let Some(contract) = self.by_artifact.get(&identity).and_then(|it| it.first()) else {
                continue;
            };
            self.admitted
                .entry(specifier)
                .or_insert_with(|| contract.clone());
        }
        self
    }

    /// The acceptances this project reaches by artifact rather than by
    /// importer.
    ///
    /// `semantic_identity` answers "what did this project import under a
    /// contract certified from one of its own files", which is the wrong
    /// question for an acceptance admitted from an installed-tree match — and
    /// the only question that had an answer while every acceptance was also
    /// importer-keyed. A report that enumerates what the analysis used has to
    /// ask both.
    pub fn admitted_contracts(&self) -> impl Iterator<Item = (&str, &AcceptedContract)> {
        self.admitted
            .iter()
            .map(|(specifier, contract)| (specifier.as_str(), contract))
    }

    #[must_use]
    pub fn contract_for_artifact(&self, artifact_identity: &str) -> Option<&AcceptedContract> {
        self.by_artifact
            .get(artifact_identity)
            .and_then(|contracts| contracts.first())
    }

    /// Enumerates the runtime surface of the one receipt-authenticated
    /// artifact case selected for this exact import occurrence.
    ///
    /// This is the only safe way for a consumer to expand an external
    /// `export *`: package names alone cannot select between nested installs,
    /// conditions, entrypoints, or artifact cases.
    pub fn public_export_names(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Result<BTreeSet<String>, SemanticQueryError> {
        Ok(self
            .contract(importer, specifier)?
            .artifact_case()
            .exports
            .keys()
            .cloned()
            .collect())
    }
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}

/// One finite Type Facts answer. `complete` says the listed possibilities are
/// exhaustive; an empty open set is unknown, never absence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FiniteFact<T> {
    possibilities: BTreeSet<T>,
    complete: bool,
}

impl<T: Ord> FiniteFact<T> {
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            possibilities: BTreeSet::new(),
            complete: false,
        }
    }

    #[must_use]
    pub fn exact(value: T) -> Self {
        Self {
            possibilities: BTreeSet::from([value]),
            complete: true,
        }
    }

    #[must_use]
    pub fn possibilities(values: impl IntoIterator<Item = T>, complete: bool) -> Self {
        Self {
            possibilities: values.into_iter().collect(),
            complete,
        }
    }

    fn evaluate(&self, expected: &T) -> GuardTruth {
        if self.complete && !self.possibilities.contains(expected) {
            GuardTruth::False
        } else if self.complete
            && self.possibilities.len() == 1
            && self.possibilities.contains(expected)
        {
            GuardTruth::True
        } else {
            GuardTruth::Unknown
        }
    }
}

impl<T: Ord> Default for FiniteFact<T> {
    fn default() -> Self {
        Self::unknown()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PropertyFact {
    pub present: FiniteFact<bool>,
    pub callable: FiniteFact<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ArgumentFacts {
    literals: FiniteFact<Literal>,
    kinds: FiniteFact<ValueKind>,
    properties: BTreeMap<String, PropertyFact>,
}

/// Exact, demand-shaped Type Facts for one call expression. Facts are local to
/// their argument/path leaf so an unresolved nested property cannot contaminate
/// a selected signature or sibling literal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallSiteFacts {
    pub selected_signatures: FiniteFact<String>,
    pub argument_counts: FiniteFact<u16>,
    arguments: BTreeMap<(u16, Vec<String>), ArgumentFacts>,
    pub tuple_alternatives: BTreeMap<u16, FiniteFact<u16>>,
    pub result_protocols: FiniteFact<ValueKind>,
}

impl CallSiteFacts {
    /// Adapts the exact portions of a Type Facts invocation transcript. Actual
    /// argument leaves are supplied separately from demanded entity rows.
    #[must_use]
    pub fn from_invocation(transcript: &typefacts::InvocationTranscript) -> Self {
        use typefacts::{FinitePartitionAxis, InvocationDomain, ValueProtocol};

        let signature_complete = transcript
            .completeness
            .contains(InvocationDomain::Signature);
        let selected_signatures = transcript.selected_signature.as_ref().map_or_else(
            || FiniteFact::possibilities([], signature_complete),
            |signature| {
                FiniteFact::possibilities([signature.identity.to_string()], signature_complete)
            },
        );
        let bindings_complete = transcript.completeness.contains(InvocationDomain::Bindings);
        let count = bindings_complete.then(|| {
            transcript
                .bindings
                .iter()
                .flat_map(|binding| binding.slots.iter())
                .map(|slot| slot.expanded_index)
                .max()
                .map_or(0, |last| last.saturating_add(1))
        });
        let argument_counts = count
            .and_then(|count| u16::try_from(count).ok())
            .map_or_else(FiniteFact::unknown, FiniteFact::exact);

        let result_protocols = transcript
            .selected_signature
            .as_ref()
            .and_then(|signature| {
                signature
                    .result
                    .partitions
                    .iter()
                    .find(|partition| partition.axis == FinitePartitionAxis::Protocol)
            })
            .map_or_else(FiniteFact::unknown, |partition| {
                FiniteFact::possibilities(
                    partition
                        .cases
                        .iter()
                        .filter_map(|case| match case.protocol? {
                            ValueProtocol::Plain => Some(ValueKind::Plain),
                            ValueProtocol::Promise => Some(ValueKind::Promise),
                            ValueProtocol::AsyncIterable => Some(ValueKind::AsyncIterable),
                        }),
                    partition.complete,
                )
            });
        Self {
            selected_signatures,
            argument_counts,
            result_protocols,
            ..Self::default()
        }
    }

    pub fn set_literal(&mut self, argument: u16, path: Vec<String>, fact: FiniteFact<Literal>) {
        self.arguments.entry((argument, path)).or_default().literals = fact;
    }

    pub fn set_value_kind(
        &mut self,
        argument: u16,
        path: Vec<String>,
        fact: FiniteFact<ValueKind>,
    ) {
        self.arguments.entry((argument, path)).or_default().kinds = fact;
    }

    pub fn set_property(
        &mut self,
        argument: u16,
        path: Vec<String>,
        name: String,
        fact: PropertyFact,
    ) {
        self.arguments
            .entry((argument, path))
            .or_default()
            .properties
            .insert(name, fact);
    }

    /// Adds one exact demanded Type Facts entity row. Exact compiler constants
    /// close the literal leaf; bounded literal candidates remain partial
    /// because the producer explicitly does not claim they are exhaustive.
    /// Runtime `other` spans plain, promise, and async-iterable values, so it
    /// expands to all three rather than manufacturing a narrower protocol.
    pub fn set_argument_entity(
        &mut self,
        argument: u16,
        path: Vec<String>,
        entity: &typefacts::EntityFact,
    ) {
        use typefacts::{ConstantValueKind, PrimitiveLiteralKind};

        let exact_literal = entity
            .constant_value
            .as_ref()
            .map(|constant| match constant.kind {
                ConstantValueKind::String => Literal::String(constant.string.to_string()),
                ConstantValueKind::Number => Literal::Number(constant.number.to_string()),
            });
        let literal_fact = exact_literal.map_or_else(
            || {
                entity.primitive_literal_candidates.as_ref().map_or_else(
                    FiniteFact::unknown,
                    |candidates| {
                        FiniteFact::possibilities(
                            candidates.iter().map(|candidate| match candidate.kind {
                                PrimitiveLiteralKind::String => {
                                    Literal::String(candidate.string.to_string())
                                }
                                PrimitiveLiteralKind::Number => {
                                    Literal::Number(candidate.number.to_string())
                                }
                                PrimitiveLiteralKind::Boolean => Literal::Bool(candidate.boolean),
                            }),
                            false,
                        )
                    },
                )
            },
            FiniteFact::exact,
        );
        self.set_literal(argument, path.clone(), literal_fact);

        let kind_fact = entity
            .runtime_value_domain
            .map_or_else(FiniteFact::unknown, |domain| {
                let mut kinds = Vec::new();
                if domain.may_be_callable() {
                    kinds.push(ValueKind::Callable);
                }
                if domain.may_be_undefined() {
                    kinds.push(ValueKind::Plain);
                }
                if domain.may_be_other() {
                    kinds.extend([
                        ValueKind::Plain,
                        ValueKind::Promise,
                        ValueKind::AsyncIterable,
                    ]);
                }
                FiniteFact::possibilities(kinds, !domain.unknown())
            });
        self.set_value_kind(argument, path, kind_fact);
    }

    fn evaluate(&self, atom: &GuardAtom, selected_case: &str) -> GuardTruth {
        match atom {
            GuardAtom::Signature(signature) => self.selected_signatures.evaluate(signature),
            GuardAtom::ArgumentCount { min, max } => {
                evaluate_numeric_range(&self.argument_counts, *min, max.unwrap_or(u16::MAX))
            }
            GuardAtom::Literal {
                argument,
                path,
                value,
            } => self
                .arguments
                .get(&(*argument, path.clone()))
                .map_or(GuardTruth::Unknown, |facts| facts.literals.evaluate(value)),
            GuardAtom::ValueKind {
                argument,
                path,
                kind,
            } => self
                .arguments
                .get(&(*argument, path.clone()))
                .map_or(GuardTruth::Unknown, |facts| facts.kinds.evaluate(kind)),
            GuardAtom::Property {
                argument,
                path,
                name,
                callable,
            } => self
                .arguments
                .get(&(*argument, path.clone()))
                .and_then(|facts| facts.properties.get(name))
                .map_or(GuardTruth::Unknown, |property| {
                    combine_truth(
                        property.present.evaluate(&true),
                        callable.map_or(GuardTruth::True, |expected| {
                            property.callable.evaluate(&expected)
                        }),
                    )
                }),
            GuardAtom::TupleAlternative {
                argument,
                alternative,
            } => self
                .tuple_alternatives
                .get(argument)
                .map_or(GuardTruth::Unknown, |facts| facts.evaluate(alternative)),
            GuardAtom::ResultProtocol(protocol) => self.result_protocols.evaluate(protocol),
            GuardAtom::ArtifactCase(case) => {
                FiniteFact::exact(selected_case.to_owned()).evaluate(case)
            }
        }
    }
}

fn evaluate_numeric_range(fact: &FiniteFact<u16>, min: u16, max: u16) -> GuardTruth {
    if fact.possibilities.is_empty() {
        return GuardTruth::Unknown;
    }
    let inside = fact
        .possibilities
        .iter()
        .filter(|value| **value >= min && **value <= max)
        .count();
    if fact.complete && inside == 0 {
        GuardTruth::False
    } else if fact.complete && inside == fact.possibilities.len() {
        GuardTruth::True
    } else {
        GuardTruth::Unknown
    }
}

fn combine_truth(left: GuardTruth, right: GuardTruth) -> GuardTruth {
    match (left, right) {
        (GuardTruth::False, _) | (_, GuardTruth::False) => GuardTruth::False,
        (GuardTruth::True, GuardTruth::True) => GuardTruth::True,
        _ => GuardTruth::Unknown,
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OpenDomainReason {
    Claim(ClaimPath),
    GuardSelection,
    OperationGuard(OperationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenDomainDiagnostic {
    pub code: &'static str,
    pub claim: Option<ClaimPath>,
    pub operation: Option<OperationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstantiatedClaim<T> {
    pub knowledge: KnowledgeSet<T>,
    pub open_reasons: Vec<OpenDomainReason>,
}

pub struct InstantiatedExport<'contract, 'facts> {
    selected_case: &'contract str,
    export: &'contract ExportSemantics,
    facts: &'facts CallSiteFacts,
}

impl InstantiatedExport<'_, '_> {
    #[must_use]
    pub const fn semantics(&self) -> &ExportSemantics {
        self.export
    }

    #[must_use]
    pub fn operation_claim(&self, domain: ClaimDomain) -> InstantiatedClaim<OperationId> {
        let Some(base) = self.export.operation_claim(domain) else {
            return InstantiatedClaim {
                knowledge: KnowledgeSet::Unknown,
                open_reasons: vec![OpenDomainReason::Claim(ClaimPath::Call(domain))],
            };
        };
        instantiate_operations(self.selected_case, self.export, base, self.facts, domain)
    }

    #[must_use]
    pub fn callbacks(&self) -> InstantiatedClaim<CallbackInvocation> {
        let selected = selected_operations(self.selected_case, self.export, self.facts);
        let mut guards_complete = true;
        let mut operation_guard_reasons = Vec::new();
        let mut items = self
            .export
            .callbacks()
            .items()
            .iter()
            .filter(|callback| {
                if !selected.knowledge.items().contains(&callback.operation) {
                    return false;
                }
                let Some(operation) = self.export.operation(&callback.operation.0) else {
                    return false;
                };
                match operation
                    .guard
                    .as_ref()
                    .map(|guard| evaluate_guard(guard, self.facts, self.selected_case))
                {
                    Some(GuardTruth::False) => false,
                    Some(GuardTruth::Unknown) => {
                        guards_complete = false;
                        operation_guard_reasons
                            .push(OpenDomainReason::OperationGuard(callback.operation.clone()));
                        true
                    }
                    Some(GuardTruth::True) | None => true,
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort();
        let complete = self.export.callbacks().is_closed()
            && selected.knowledge.is_closed()
            && guards_complete;
        let knowledge = knowledge_from(items, complete);
        let mut open_reasons = selected.open_reasons;
        open_reasons.extend(operation_guard_reasons);
        if !self.export.callbacks().is_closed() {
            open_reasons.push(OpenDomainReason::Claim(ClaimPath::Call(
                ClaimDomain::Callbacks,
            )));
        }
        canonicalize_reasons(&mut open_reasons);
        InstantiatedClaim {
            knowledge,
            open_reasons,
        }
    }

    #[must_use]
    pub fn possible_operations(&self, domain: ClaimDomain) -> Vec<&Operation> {
        self.operation_claim(domain)
            .knowledge
            .items()
            .iter()
            .filter_map(|id| self.export.operation(&id.0))
            .collect()
    }

    #[must_use]
    pub fn guaranteed_operations(&self, domain: ClaimDomain) -> Vec<&Operation> {
        let instantiated = self.operation_claim(domain);
        if instantiated.open_reasons.iter().any(|reason| {
            matches!(
                reason,
                OpenDomainReason::GuardSelection | OpenDomainReason::OperationGuard(_)
            )
        }) {
            return Vec::new();
        }
        instantiated
            .knowledge
            .items()
            .iter()
            .filter_map(|id| self.export.operation(&id.0))
            .filter(|operation| operation.cardinality.strength() == BehaviorStrength::Guaranteed)
            .collect()
    }
}

fn instantiate_operations(
    selected_case: &str,
    export: &ExportSemantics,
    base: &KnowledgeSet<OperationId>,
    facts: &CallSiteFacts,
    domain: ClaimDomain,
) -> InstantiatedClaim<OperationId> {
    let selected = selected_operations(selected_case, export, facts);
    let mut reasons = selected.open_reasons;
    if !base.is_closed() {
        reasons.push(OpenDomainReason::Claim(ClaimPath::Call(domain)));
    }
    let mut items = Vec::new();
    let mut guards_complete = true;
    for id in base.items() {
        if !selected.knowledge.items().contains(id) {
            continue;
        }
        let Some(operation) = export.operation(&id.0) else {
            continue;
        };
        match operation
            .guard
            .as_ref()
            .map(|guard| evaluate_guard(guard, facts, selected_case))
        {
            Some(GuardTruth::False) => {}
            Some(GuardTruth::Unknown) => {
                items.push(id.clone());
                guards_complete = false;
                reasons.push(OpenDomainReason::OperationGuard(id.clone()));
            }
            Some(GuardTruth::True) | None => items.push(id.clone()),
        }
    }
    items.sort();
    items.dedup();
    let complete = base.is_closed() && selected.knowledge.is_closed() && guards_complete;
    canonicalize_reasons(&mut reasons);
    InstantiatedClaim {
        knowledge: knowledge_from(items, complete),
        open_reasons: reasons,
    }
}

fn selected_operations(
    selected_case: &str,
    export: &ExportSemantics,
    facts: &CallSiteFacts,
) -> InstantiatedClaim<OperationId> {
    if matches!(export.call.guards.cases, KnowledgeSet::Unknown) {
        return InstantiatedClaim {
            knowledge: KnowledgeSet::complete(
                export
                    .call
                    .operations
                    .iter()
                    .map(|operation| operation.id.clone())
                    .collect(),
            ),
            open_reasons: Vec::new(),
        };
    }
    let knowledge = export
        .call
        .guards
        .select_operations(|atom| facts.evaluate(atom, selected_case));
    let unresolved_selection = export
        .call
        .guards
        .cases
        .items()
        .iter()
        .any(|case| match case {
            GuardedCase::When { guard, .. } => {
                evaluate_guard(guard, facts, selected_case) == GuardTruth::Unknown
            }
            GuardedCase::Otherwise { .. } => false,
        });
    let open_reasons = (!knowledge.is_closed() || unresolved_selection)
        .then_some(OpenDomainReason::GuardSelection)
        .into_iter()
        .collect();
    InstantiatedClaim {
        knowledge,
        open_reasons,
    }
}

fn evaluate_guard(guard: &Guard, facts: &CallSiteFacts, selected_case: &str) -> GuardTruth {
    let mut unknown = false;
    for atom in &guard.0 {
        match facts.evaluate(atom, selected_case) {
            GuardTruth::False => return GuardTruth::False,
            GuardTruth::Unknown => unknown = true,
            GuardTruth::True => {}
        }
    }
    if unknown {
        GuardTruth::Unknown
    } else {
        GuardTruth::True
    }
}

fn knowledge_from<T>(items: Vec<T>, complete: bool) -> KnowledgeSet<T> {
    if complete {
        KnowledgeSet::Complete(items)
    } else if items.is_empty() {
        KnowledgeSet::Unknown
    } else {
        KnowledgeSet::Partial(items)
    }
}

fn canonicalize_reasons(reasons: &mut Vec<OpenDomainReason>) {
    reasons.sort();
    reasons.dedup();
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SemanticQueryError {
    #[error("no accepted contract for exact import {specifier:?} from {importer:?}")]
    MissingImport { importer: String, specifier: String },
    #[error("multiple accepted contracts claim exact import {specifier:?} from {importer:?}")]
    AmbiguousImport { importer: String, specifier: String },
    #[error("accepted contract has no exact export identity for {export}")]
    MissingExport { export: String },
    #[error("accepted contract contains duplicate exact export identity for {export}")]
    AmbiguousExport { export: String },
    #[error("native dialect and accepted contract contradict claim domain {domain:?}")]
    NativeContractConflict { domain: ClaimDomain },
}

impl OpenDomainReason {
    #[must_use]
    pub fn diagnostic(&self) -> OpenDomainDiagnostic {
        match self {
            Self::Claim(path) => OpenDomainDiagnostic {
                code: "open-claim-domain",
                claim: Some(path.clone()),
                operation: None,
            },
            Self::GuardSelection => OpenDomainDiagnostic {
                code: "unresolved-guard-selection",
                claim: None,
                operation: None,
            },
            Self::OperationGuard(operation) => OpenDomainDiagnostic {
                code: "unresolved-operation-guard",
                claim: None,
                operation: Some(operation.clone()),
            },
        }
    }
}

impl<T> InstantiatedClaim<T> {
    #[must_use]
    pub fn diagnostics(&self) -> Vec<OpenDomainDiagnostic> {
        self.open_reasons
            .iter()
            .map(OpenDomainReason::diagnostic)
            .collect()
    }
}

pub(super) fn resolve_export<'a>(
    contract: &'a AcceptedContract,
    identity: &ExportIdentity,
) -> Result<&'a ExportSemantics, SemanticQueryError> {
    let matches = contract
        .artifact_case()
        .exports
        .values()
        .filter(|export| export.identity == *identity)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(SemanticQueryError::MissingExport {
            export: identity.public_name.clone(),
        }),
        [export] => Ok(*export),
        _ => Err(SemanticQueryError::AmbiguousExport {
            export: identity.public_name.clone(),
        }),
    }
}

pub(super) fn instantiate_export<'contract, 'facts>(
    selected_case: &'contract str,
    export: &'contract ExportSemantics,
    facts: &'facts CallSiteFacts,
) -> Result<InstantiatedExport<'contract, 'facts>, SemanticQueryError> {
    Ok(InstantiatedExport {
        selected_case,
        export,
        facts,
    })
}

/// Native dialect semantics outrank a compatible accepted contract. A proved
/// contradiction is refused rather than picking the friendlier answer. Open
/// knowledge on either side cannot manufacture a conflict or negative proof.
pub fn native_claim_precedence<T: Ord + Clone>(
    domain: ClaimDomain,
    native: Option<&KnowledgeSet<T>>,
    contract: &KnowledgeSet<T>,
) -> Result<KnowledgeSet<T>, SemanticQueryError> {
    let Some(native) = native else {
        return Ok(contract.clone());
    };
    let conflict = (native.is_closed()
        && contract
            .items()
            .iter()
            .any(|item| !native.items().contains(item)))
        || (contract.is_closed()
            && native
                .items()
                .iter()
                .any(|item| !contract.items().contains(item)));
    if conflict {
        Err(SemanticQueryError::NativeContractConflict { domain })
    } else {
        Ok(native.clone())
    }
}

#[cfg(test)]
mod tests;
