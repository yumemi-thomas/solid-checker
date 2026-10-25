//! ADRs 0266/0268: an assumed runtime condition, with positive visible vetoes only.

use crate::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use solid_dialect::{Dialect, RuntimeConfigurationApi};
use solid_facts::ast::ImportKind;
use solid_facts::core::Span;
use solid_facts::{ProjectFacts, TypeScriptTable};
use typefacts::Location;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum RuntimeConfigurationPremise {
    #[default]
    Assumed,
    Vetoed {
        location: Location,
    },
}

impl RuntimeConfigurationPremise {
    pub fn permits_proof(&self) -> bool {
        matches!(self, Self::Assumed)
    }

    pub(crate) fn apply(&self, finding: &mut crate::Finding) {
        let Self::Vetoed { location } = self else {
            // In particular, do not add evidence to existing Assumed findings.
            return;
        };
        finding.kind = "uncertifiable".into();
        finding.message = format!(
            "Cannot certify this finding under the standard runtime configuration premise: a configuration export is used at {}:{}-{}.",
            location.path, location.start_byte, location.end_byte
        );
        finding.hint = "Review ADRs 0266/0268 and the configuration site; restoring the premise requires reanalysis of the configured project.".into();
        finding.fixes.clear();
        finding.evidence.clear();
        finding.evidence.push(crate::EvidenceStep {
            message: "Standard runtime configuration premise vetoed (ADR 0266).".into(),
            location: Some(location.clone()),
        });
        finding.related_locations.push(location.clone());
    }
}

/// Follow exact binder aliases. Missing, cyclic, synthetic or merged evidence
/// supplies no positive identity and therefore cannot veto this premise.
///
/// `chain` receives every symbol whose row the answer read: the identity is a
/// function of those rows, the package roots and the dialect alone.
fn configuration_api(
    table: &TypeScriptTable,
    symbol: &str,
    dialect: &dyn Dialect,
    package_roots: &[(String, String)],
    chain: &mut Vec<String>,
) -> Option<RuntimeConfigurationApi> {
    let (path, start, end) = canonical_symbol_declaration(table, symbol, package_roots, chain)?;
    dialect.runtime_configuration_api(&path, start, end)
}

fn canonical_symbol_declaration(
    table: &TypeScriptTable,
    symbol: &str,
    package_roots: &[(String, String)],
    chain: &mut Vec<String>,
) -> Option<(String, u64, u64)> {
    let mut current = symbol;
    let mut seen = HashSet::default();
    loop {
        if !seen.insert(current) {
            return None;
        }
        chain.push(current.to_owned());
        let resolved = table.symbol(current)?;
        if !resolved.alias_target().is_empty() {
            current = resolved.alias_target();
            continue;
        }
        let [declaration] = resolved.declarations() else {
            return None;
        };
        let path = canonical_declaration_path(declaration.location.path.as_ref(), package_roots)?;
        return Some((
            path,
            declaration.location.start_byte,
            declaration.location.end_byte,
        ));
    }
}

/// Normalize npm aliases/linked roots only from existing attested manifests.
/// Conflicting owning identities do not manufacture positive identification.
fn canonical_declaration_path(path: &str, roots: &[(String, String)]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut best: Option<(usize, String)> = None;
    for (root, package) in roots {
        let Some(relative) = normalized
            .strip_prefix(root)
            .and_then(|tail| tail.strip_prefix('/'))
        else {
            continue;
        };
        let canonical = format!("/node_modules/{package}/{relative}");
        match &best {
            Some((length, previous)) if *length == root.len() && previous != &canonical => {
                return None;
            }
            Some((length, _)) if *length > root.len() => {}
            _ => best = Some((root.len(), canonical)),
        }
    }
    Some(best.map_or(normalized, |(_, path)| path))
}

fn runtime_span(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    !ast.type_queries.iter().any(|query| query.contains(span))
        && !ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
}

/// `if (DEV)`, `DEV ? a : b` and `DEV && f()` only test the binding's
/// truthiness: none can reach a hook or exclusion. Every other reference,
/// including `DEV.hooks` and passing `DEV` on, still vetoes. Without at least
/// one binder-resolved reference the use is not established as harmless.
fn only_truthiness_tests(ast: &solid_facts::ast::AstFacts, declaration: Span) -> bool {
    use solid_facts::ast::{IdentifierRole, LogicalOperatorKind};
    let mut references = ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && ast.reference_declaration(id.span) == Some(declaration)
        })
        .peekable();
    references.peek().is_some()
        && references.all(|id| {
            !runtime_span(ast, id.span)
                || ast.if_regions.iter().any(|region| region.test == id.span)
                || ast
                    .conditional_expressions
                    .iter()
                    .any(|conditional| conditional.test == id.span)
                || ast.logical_expressions.iter().any(|logical| {
                    logical.operator == LogicalOperatorKind::And && logical.left == id.span
                })
        })
}

fn truthiness_use(ast: &solid_facts::ast::AstFacts, expression: Span) -> bool {
    use solid_facts::ast::{CoerciveOperandKind, LogicalOperatorKind};
    let is_expression = |span| ast.peel_ts_sugar_span(span) == expression;
    ast.if_regions
        .iter()
        .any(|region| is_expression(region.test))
        || ast
            .conditional_expressions
            .iter()
            .any(|conditional| is_expression(conditional.test))
        || ast.logical_expressions.iter().any(|logical| {
            logical.operator == LogicalOperatorKind::And && is_expression(logical.left)
        })
        || ast.coercive_operands.iter().any(|operand| {
            operand.kind == CoerciveOperandKind::LogicalNot && is_expression(operand.span)
        })
}

/// The host identity is already exact. Only its direct static data reads or
/// truthiness can avoid a veto; callback reads and object transport cannot.
fn harmless_hydration_host_use(
    ast: &solid_facts::ast::AstFacts,
    source: &str,
    expression: Span,
    dialect: &dyn Dialect,
) -> bool {
    if !runtime_span(ast, expression) {
        return true;
    }
    if truthiness_use(ast, expression) {
        return true;
    }
    let Some(member) = ast.members.iter().find(|member| {
        ast.peel_ts_sugar_span(member.object) == expression
            && !ast.computed_members.contains(&member.span)
    }) else {
        return false;
    };
    // Containment is not a write: computed keys and destructuring defaults
    // can read a harmless flag while addressing a different target.
    if ast.write_targets.contains(&member.span)
        || ast
            .deleted_targets
            .iter()
            .any(|target| ast.peel_ts_sugar_span(*target) == member.span)
    {
        return false;
    }
    let Some(name) = source.get(member.property.start as usize..member.property.end as usize)
    else {
        return false;
    };
    dialect.hydration_host_member_read_is_harmless(name, truthiness_use(ast, member.span))
}

fn only_harmless_hydration_host_uses(
    ast: &solid_facts::ast::AstFacts,
    source: &str,
    declaration: Span,
    dialect: &dyn Dialect,
) -> bool {
    use solid_facts::ast::IdentifierRole;
    let mut references = ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && ast.reference_declaration(id.span) == Some(declaration)
        })
        .peekable();
    references.peek().is_some()
        && references.all(|id| harmless_hydration_host_use(ast, source, id.span, dialect))
}

fn package_roots(facts: &ProjectFacts) -> Vec<(String, String)> {
    let mut roots = facts
        .resolved_imports
        .iter()
        .flat_map(|resolutions| resolutions.iter())
        .filter_map(|(_, import)| {
            let manifest = import.package_manifest.as_ref()?.replace('\\', "/");
            let (root, _) = manifest.rsplit_once('/')?;
            Some((root.to_owned(), import.package_name.as_ref()?.to_string()))
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

/// ADR 0284: a file of a package analyzed as source. Its own use of `DEV`,
/// `OBSERVE` or a hydration host is the package's development tooling, not
/// the project's runtime configuration, so it vetoes nothing.
fn is_dependency_source(facts: &ProjectFacts, file: &solid_facts::FileFacts) -> bool {
    facts.dependency_source_root(file.path.as_str()).is_some()
}

pub(crate) fn scan(facts: &ProjectFacts, dialect: &dyn Dialect) -> RuntimeConfigurationPremise {
    scan_retained(facts, dialect, &mut ScanCache::default())
}

/// Each file's part of [`scan`], retained across generations. A file's answer
/// is a function of its source, its own Type Facts entities, and the
/// configuration identity of each symbol its lookups asked about. Reuse
/// checks the first two against the current facts, and keeps each asked
/// symbol's identity with the symbols its alias chain read: one is asked
/// again only when a row it read differs between the table it was read from
/// and the current one ([`TypeScriptTable::symbols_with_moved_declarations`]),
/// never on the word of a change set, which may describe a table other than
/// the one this cache read (a cancelled exchange).
#[derive(Default)]
pub(crate) struct ScanCache {
    basis: Option<ScanBasis>,
    /// The table the retained answers were read from: a file's entities are
    /// compared with the ones read there, and its symbol rows with the
    /// current table's.
    table: Option<TypeScriptTable>,
    files: HashMap<String, FileScan>,
    /// The identity in `table` of every symbol a kept scan asked about.
    identities: HashMap<String, Identity>,
    /// For each symbol some identity's chain read, the symbols whose chain
    /// read it.
    chain_readers: HashMap<String, HashSet<String>>,
    /// For each asked symbol, the files whose kept scan asked it.
    askers: HashMap<String, HashSet<String>>,
    /// The last scan's (reused, scanned) file counts, for timings.
    pub(crate) last_counts: (u64, u64),
}

#[derive(Eq, PartialEq)]
struct ScanBasis {
    dialect: solid_dialect::Version,
    project_id: String,
    roots: Vec<(String, String)>,
    dependency_roots: Vec<String>,
}

struct FileScan {
    source_hash: solid_facts::core::SourceHash,
    /// Each symbol the file's lookups resolved, sorted.
    asked: Vec<String>,
    veto: Option<Location>,
}

/// A symbol's configuration identity and the symbols whose rows it read.
struct Identity {
    api: Option<RuntimeConfigurationApi>,
    chain: Vec<String>,
}

impl ScanCache {
    /// `symbol`'s identity in `table`: the kept one, else computed and kept.
    fn identity(
        &mut self,
        table: &TypeScriptTable,
        dialect: &dyn Dialect,
        roots: &[(String, String)],
        symbol: &str,
    ) -> Option<RuntimeConfigurationApi> {
        if let Some(identity) = self.identities.get(symbol) {
            return identity.api;
        }
        let mut chain = Vec::new();
        let api = configuration_api(table, symbol, dialect, roots, &mut chain);
        for read in &chain {
            self.chain_readers
                .entry(read.clone())
                .or_default()
                .insert(symbol.to_owned());
        }
        self.identities
            .insert(symbol.to_owned(), Identity { api, chain });
        api
    }

    /// Drops `symbol`'s kept identity, returning it.
    fn forget(&mut self, symbol: &str) -> Option<Identity> {
        let identity = self.identities.remove(symbol)?;
        for read in &identity.chain {
            if let Some(readers) = self.chain_readers.get_mut(read.as_str()) {
                readers.remove(symbol);
                if readers.is_empty() {
                    self.chain_readers.remove(read.as_str());
                }
            }
        }
        Some(identity)
    }

    /// Drops the kept scan of `path`, and every identity no kept scan asks
    /// about any more.
    fn drop_file(&mut self, path: &str) {
        let Some(scan) = self.files.remove(path) else {
            return;
        };
        for symbol in &scan.asked {
            let Some(files) = self.askers.get_mut(symbol.as_str()) else {
                continue;
            };
            files.remove(path);
            if files.is_empty() {
                self.askers.remove(symbol.as_str());
                self.forget(symbol);
            }
        }
    }
}

/// [`scan`], reusing a file's retained answer only under the same dialect,
/// project, package roots and dependency-source roots, when the file's
/// source hash is unchanged, its entities equal the ones read before, and no
/// symbol it asked about changed identity. The answer is the first veto in
/// file order, as a full scan's.
pub(crate) fn scan_retained(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    cache: &mut ScanCache,
) -> RuntimeConfigurationPremise {
    let basis = ScanBasis {
        dialect: dialect.version(),
        project_id: facts.project_id.clone(),
        roots: package_roots(facts),
        dependency_roots: facts.dependency_source_roots.clone(),
    };
    if cache.basis.as_ref() != Some(&basis) || cache.table.is_none() {
        *cache = ScanCache::default();
    }
    let previous_table = cache.table.take();
    let table = &facts.typescript;
    // The kept identities whose chain read a row that moved, asked again.
    let mut changed = HashSet::default();
    if let Some(previous) = &previous_table {
        let stale = table
            .symbols_with_moved_declarations(previous)
            .iter()
            .filter_map(|moved| cache.chain_readers.get(moved.as_ref()))
            .flatten()
            .cloned()
            .collect::<HashSet<_>>();
        for symbol in stale {
            let before = cache.forget(&symbol).and_then(|identity| identity.api);
            if cache.identity(table, dialect, &basis.roots, &symbol) != before {
                changed.insert(symbol);
            }
        }
    }
    let scanned_paths = facts
        .files
        .iter()
        .filter(|file| !is_dependency_source(facts, file))
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let removed = cache
        .files
        .keys()
        .filter(|path| !scanned_paths.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for path in removed {
        cache.drop_file(&path);
    }
    let mut rescan = changed
        .iter()
        .filter_map(|symbol| cache.askers.get(symbol.as_str()))
        .flatten()
        .cloned()
        .collect::<HashSet<_>>();
    let mut premise = RuntimeConfigurationPremise::Assumed;
    let mut counts = (0, 0);
    for file in facts
        .files
        .iter()
        .filter(|file| !is_dependency_source(facts, file))
    {
        let path = file.path.as_str();
        let retained = !rescan.contains(path)
            && previous_table.as_ref().is_some_and(|previous| {
                cache.files.get(path).is_some_and(|scan| {
                    scan.source_hash == file.source_hash
                        && same_scanned_entities(
                            previous.entities_for_path(path),
                            table.entities_for_path(path),
                        )
                })
            });
        if retained {
            counts.0 += 1;
        } else {
            counts.1 += 1;
            cache.drop_file(path);
            let mut asked = Vec::new();
            let veto = scan_file(
                facts,
                file,
                dialect,
                &mut |symbol| cache.identity(table, dialect, &basis.roots, symbol),
                &mut asked,
            );
            asked.sort_unstable();
            asked.dedup();
            for symbol in &asked {
                cache
                    .askers
                    .entry(symbol.clone())
                    .or_default()
                    .insert(path.to_owned());
            }
            cache.files.insert(
                path.to_owned(),
                FileScan {
                    source_hash: file.source_hash.clone(),
                    asked,
                    veto,
                },
            );
            // A path listed twice is scanned once (its slots carry one
            // source).
            rescan.remove(path);
        }
        if premise == RuntimeConfigurationPremise::Assumed
            && let Some(location) = cache.files.get(path).and_then(|scan| scan.veto.as_ref())
        {
            premise = RuntimeConfigurationPremise::Vetoed {
                location: location.clone(),
            };
        }
    }
    cache.basis = Some(basis);
    cache.table = Some(table.clone());
    cache.last_counts = counts;
    premise
}

/// Whether two entity lists agree on everything [`scan_file`] reads of them:
/// each entity's exact span, symbol and whether it is unresolved, in order.
fn same_scanned_entities(left: &[typefacts::EntityFact], right: &[typefacts::EntityFact]) -> bool {
    std::ptr::eq(left, right)
        || left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                left.location.start_byte == right.location.start_byte
                    && left.location.end_byte == right.location.end_byte
                    && left.symbol_claim == right.symbol_claim
                    && left.symbol == right.symbol
            })
}

/// One file's first veto, if any; `asked` receives every symbol resolved and
/// the identity it had.
fn scan_file(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    dialect: &dyn Dialect,
    identity: &mut dyn FnMut(&str) -> Option<RuntimeConfigurationApi>,
    asked: &mut Vec<String>,
) -> Option<Location> {
    // Index only entities already supplied by the analysis. A duplicate
    // exact span is ambiguous; contained entities are never substitutes.
    let mut symbols = HashMap::default();
    for entity in facts.typescript.entities_for_path(file.path.as_str()) {
        let key = (entity.location.start_byte, entity.location.end_byte);
        let symbol = (!entity.symbol_unresolved() && !entity.symbol.is_empty())
            .then_some(entity.symbol.as_ref());
        symbols
            .entry(key)
            .and_modify(|value| *value = None)
            .or_insert(symbol);
    }
    let mut api_at = |span: Span| {
        let symbol = symbols
            .get(&(u64::from(span.start), u64::from(span.end)))
            .copied()
            .flatten()?;
        asked.push(symbol.to_owned());
        identity(symbol)
    };
    let member_properties = file
        .ast
        .members
        .iter()
        .map(|member| (member.span, member.property))
        .collect::<HashMap<_, _>>();
    // The existing Oxc binder proves a value use, excluding type-only
    // imports and value-as-type (`typeof DEV`) queries. No Type Facts
    // reference list is fetched or required, including for namespaces.
    for import in &file.ast.imports {
        if import.type_only {
            continue;
        }
        for binding in &import.bindings {
            if binding.type_only
                || !binding.runtime_referenced
                || matches!(binding.kind, ImportKind::Namespace | ImportKind::SideEffect)
            {
                continue;
            }
            let veto = match api_at(binding.local.span) {
                Some(
                    RuntimeConfigurationApi::DevelopmentHooks
                    | RuntimeConfigurationApi::Observation,
                ) => !only_truthiness_tests(&file.ast, binding.local.span),
                Some(RuntimeConfigurationApi::HydrationHost) => !only_harmless_hydration_host_uses(
                    &file.ast,
                    &file.source,
                    binding.local.span,
                    dialect,
                ),
                _ => false,
            };
            if veto {
                return Some(binding.local.span.location(file.path.shared()));
            }
        }
    }
    // Includes exact namespace members, TS wrappers, and first references
    // before aliasing/destructuring/escape; unrelated members do not veto.
    for export in &file.ast.exports {
        if export.type_only {
            continue;
        }
        for specifier in &export.specifiers {
            if !specifier.type_only
                && api_at(specifier.local.span) == Some(RuntimeConfigurationApi::HydrationHost)
            {
                return Some(specifier.local.span.location(file.path.shared()));
            }
        }
    }
    for member in &file.ast.members {
        if runtime_span(&file.ast, member.span)
            && match api_at(member.property) {
                Some(
                    RuntimeConfigurationApi::DevelopmentHooks
                    | RuntimeConfigurationApi::Observation,
                ) => true,
                Some(RuntimeConfigurationApi::HydrationHost) => {
                    !harmless_hydration_host_use(&file.ast, &file.source, member.span, dialect)
                }
                _ => false,
            }
        {
            return Some(member.span.location(file.path.shared()));
        }
    }
    // Merely importing or reading enableExternalSource is not a call.
    for call in &file.ast.calls {
        if call.construct || !runtime_span(&file.ast, call.span) {
            continue;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let property = member_properties.get(&callee).copied();
        if api_at(call.callee)
            .or_else(|| api_at(callee))
            .or_else(|| property.and_then(&mut api_at))
            == Some(RuntimeConfigurationApi::ExternalSource)
        {
            return Some(call.span.location(file.path.shared()));
        }
    }
    None
}

#[cfg(test)]
mod tests;
