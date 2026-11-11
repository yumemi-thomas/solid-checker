//! ADR 0949, addendum 6: a package operation that a project site runs in a context where the runtime refuses it.
//!
//! The walks that decide the context-bound rules start from the operation (the flush, the creating call, the
//! cleanup) or from a project call they follow, and each stops where it cannot follow: a leaf scope's call walk
//! (SC3001) does not enter a constructor or a returned object's getter, the effect phase's origin walk (SC2008,
//! SC6002) gives up at an exported function, and the read-after-await walk (SC1002) does not follow `new`, a getter
//! or a tag into a package. The package holds no finding of those rules for the operation (a finding in a package
//! is dropped and handed to the sites that run it, ADR 0284: there was nothing to hand), so the project was certified
//! clean where Solid threw or warned (`PRIMITIVE_IN_FORBIDDEN_SCOPE`, `CLEANUP_IN_FORBIDDEN_SCOPE`,
//! `FLUSH_IN_TRACKED_QUEUE_CALLBACK`, `PRIMITIVE_IN_EFFECT_CALLBACK`, `FLUSH_IN_EFFECT_CALLBACK`,
//! `UNTRACKED_READ_AFTER_AWAIT`).
//!
//! A project site that may run a package function ([`crate::PackageEntries`], the references of
//! [`crate::PackageReferences`]) and is written in a context that makes one of the function's operations a claim of
//! a rule is that rule's uncertifiable row:
//!
//! | context at the site (lexical) | operation in the package | row |
//! | --- | --- | --- |
//! | in a leaf owner's callback (`onSettled`, `createTrackedEffect`) | `onCleanup`, a creating call, `flush` | SC3001 |
//! | in a callback an effect's apply phase runs | `flush` | SC2008 |
//! | in a callback an effect's apply phase runs | a creating call the dialect says throws there | SC6002 |
//! | after an await of an async tracked computation | a read of a reactive source | SC1002 |
//!
//! Never a violation: whether the site runs the operation (a branch, a later entry, a package that guards it) is
//! not proven here. A site the call walk of the rule already follows adds nothing: a call that enters a plain
//! function by its own name is the leaf walk's and the after-await walk's (they read its body), so the claim stands
//! only where those walks are blind: a `new`, a method, a getter or a constructor, a reference, a render. The effect
//! phase has no walk from the apply down, so every entry stands there.

use super::*;

use std::collections::BTreeSet;

use crate::{
    LeafOwnerOperation, LeafOwnerOperationKind, StaticDefect, StaticDefectKind, StaticViolation,
};
use solid_dialect::{CallbackOwner, Primitive};

/// The lexical regions of one project file in which a package operation is a claim.
#[derive(Default)]
struct FileRegions {
    /// The callback arguments a leaf owner runs: the argument's span, the owner primitive's written name.
    leaf: Vec<(Span, String)>,
    /// The callback arguments an effect's apply phase runs.
    apply: Vec<Span>,
    /// The async function literals that are the tracked compute of a computation.
    computations: Vec<Span>,
}

impl FileRegions {
    fn is_empty(&self) -> bool {
        self.leaf.is_empty() && self.apply.is_empty() && self.computations.is_empty()
    }

    fn of(file: &solid_facts::FileFacts, lookup: &SemanticLookup<'_>) -> Self {
        let dialect = lookup.dialect;
        let mut regions = Self::default();
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.spread {
                    break;
                }
                if crate::owners::callback_owner_at_call(file, call, primitive, index, lookup)
                    == Some(CallbackOwner::Leaf)
                {
                    regions.leaf.push((
                        argument.span,
                        file.source_text(call.callee).unwrap_or_default().to_owned(),
                    ));
                }
                if dialect.callback_runs_in_effect_phase(primitive, index)
                    || dialect.callback_reruns_in_effect_phase(primitive, index)
                {
                    regions.apply.push(argument.span);
                }
            }
            if let Some(argument) = call.arguments.first().filter(|argument| !argument.spread)
                && dialect
                    .callback_semantics_at(primitive, 0, call.arguments.len())
                    .tracks_reads
                && let Some(function) =
                    crate::cleanup::callback_argument_literal(file, argument.span)
                        .filter(|function| function.r#async)
            {
                regions.computations.push(function.span);
            }
        }
        regions
    }

    /// The contexts a site written at `span` of `file` may run in.
    fn contexts(&self, file: &solid_facts::FileFacts, span: Span) -> SiteContexts {
        SiteContexts {
            leaf: self
                .leaf
                .iter()
                .find(|(region, _)| region.contains(span))
                .map(|(_, owner)| owner.clone()),
            apply: self.apply.iter().any(|region| region.contains(span)),
            after_await: self.computations.iter().any(|computation| {
                crate::owners::containing_ast_function(&file.ast, span)
                    .is_some_and(|function| function.span == *computation)
            }) && crate::execution_role::may_follow_suspension(file, span),
        }
    }
}

/// The contexts a project site may run in.
#[derive(Default)]
pub(crate) struct SiteContexts {
    /// A leaf owner's callback: the owner primitive as written.
    pub(crate) leaf: Option<String>,
    /// A callback an effect's apply phase runs.
    pub(crate) apply: bool,
    /// After an await of an async tracked computation.
    pub(crate) after_await: bool,
}

impl SiteContexts {
    fn none(&self) -> bool {
        self.leaf.is_none() && !self.apply && !self.after_await
    }
}

/// Where the project's regions are, per file, and the package roots the sites in them mention (built once per
/// build; empty for a project that mentions no package in such a region).
pub(crate) struct OperationRegions<'a> {
    /// The project files that hold a region, with their regions: the facts the claims read a site's file from.
    files: HashMap<String, (&'a solid_facts::FileFacts, FileRegions)>,
    /// The roots of the packages a site in a region mentions: the only packages whose operations are censused.
    roots: BTreeSet<String>,
}

impl<'a> OperationRegions<'a> {
    pub(crate) fn build(
        facts: &'a ProjectFacts,
        lookup: &SemanticLookup<'_>,
        mentions: &PackageMentions,
    ) -> Self {
        let mut files = HashMap::<String, (&'a solid_facts::FileFacts, FileRegions)>::default();
        let mut roots = BTreeSet::<String>::new();
        if facts.dependency_source_roots.is_empty() {
            return Self { files, roots };
        }
        for file in crate::base::reads::all_files(facts) {
            if facts.dependency_source_root(file.path.as_str()).is_some()
                || !(mentions.refs.contains_key(file.path.as_str())
                    || mentions.carriers.contains_key(file.path.as_str()))
            {
                continue;
            }
            let regions = FileRegions::of(file, lookup);
            if regions.is_empty() {
                continue;
            }
            let mut mentioned = |span: Span| {
                if !regions.contexts(file, span).none() {
                    roots.extend(
                        mentions
                            .roots_in(file, span, false)
                            .into_iter()
                            .map(|root| mentions.roots[root].clone()),
                    );
                }
            };
            for call in &file.ast.calls {
                mentioned(call.span);
            }
            for element in &file.ast.jsx_elements {
                mentioned(element_site(element));
            }
            for template in &file.ast.tagged_templates {
                mentioned(template.span);
            }
            files.insert(file.path.to_string(), (file, regions));
        }
        Self { files, roots }
    }

    /// The contexts a project site at `span` of the file at `path` may run in.
    pub(crate) fn contexts(&self, path: &str, span: Span) -> SiteContexts {
        self.files
            .get(path)
            .map(|(file, regions)| regions.contexts(file, span))
            .unwrap_or_default()
    }

    /// The facts of the project file at `path`, when it holds a region.
    fn file(&self, path: &str) -> Option<&'a solid_facts::FileFacts> {
        self.files.get(path).map(|(file, _)| *file)
    }

    /// Whether no site of the project is in a region that mentions a package.
    fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// What kind of operation a package call or read is.
#[derive(Clone)]
enum Family {
    /// `onCleanup`.
    Cleanup,
    /// A creating call the leaf owner refuses, by its written name.
    Creation(String, Primitive),
    /// `flush`.
    Flush,
    /// A read of a reactive source.
    Read,
}

/// One operation written in a package analyzed as source.
pub(crate) struct PackageOperation<'a> {
    location: Location,
    family: Family,
    /// The package file it is written in.
    file: &'a solid_facts::FileFacts,
}

impl PackageOperation<'_> {
    pub(crate) fn location(&self) -> &Location {
        &self.location
    }
}

/// The operations of the packages of `regions.roots`: each call of a primitive the leaf owner or the effect phase
/// refuses (`onCleanup`, a creating call, `flush`), and each read the program holds in them.
pub(crate) fn package_operations<'a>(
    facts: &'a ProjectFacts,
    lookup: &SemanticLookup<'_>,
    regions: &OperationRegions<'_>,
    program: &crate::Program,
) -> Vec<PackageOperation<'a>> {
    let mut operations = Vec::new();
    if regions.is_empty() {
        return operations;
    }
    let dialect = lookup.dialect;
    let mut package_files = HashMap::<&str, &'a solid_facts::FileFacts>::default();
    for file in crate::base::reads::all_files(facts) {
        let Some(root) = facts.dependency_source_root(file.path.as_str()) else {
            continue;
        };
        if !regions.roots.contains(root) {
            continue;
        }
        package_files.insert(file.path.as_str(), file);
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let Some(family) = family_of(dialect, file, call, primitive) else {
                continue;
            };
            operations.push(PackageOperation {
                location: crate::location(file.path.shared(), call.callee),
                family,
                file,
            });
        }
    }
    for read in &program.reads {
        if let Some(file) = package_files.get(read.location.path.as_ref()) {
            operations.push(PackageOperation {
                location: read.location.clone(),
                family: Family::Read,
                file,
            });
        }
    }
    operations.sort_by(|left, right| crate::location_order(&left.location, &right.location));
    operations
}

/// What [`package_operation_claims`] adds to the program.
#[derive(Default)]
pub(crate) struct OperationClaims {
    pub(crate) leaf_operations: Vec<LeafOwnerOperation>,
    pub(crate) violations: Vec<StaticViolation>,
    pub(crate) defects: Vec<StaticDefect>,
}

/// The claims of the package operations at the project sites that may run them ([`crate::PackageEntries`] by the
/// innermost function holding the operation, [`crate::PackageReferences`] by the operation's package), in the
/// contexts the module names.
pub(crate) fn package_operation_claims(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    program: &crate::Program,
    regions: &OperationRegions<'_>,
    operations: &[PackageOperation<'_>],
    handed: &[StaticDefect],
) -> OperationClaims {
    let mut claims = OperationClaims::default();
    if regions.is_empty() || operations.is_empty() {
        return claims;
    }
    let dialect = lookup.dialect;
    // The entries by the path of the package function that holds them.
    let mut by_path = HashMap::<&str, Vec<&crate::PackageEntries>>::default();
    for entry in &program.package_entries {
        by_path
            .entry(entry.function.path.as_ref())
            .or_default()
            .push(entry);
    }
    let mut seen = BTreeSet::<(String, u64, String)>::new();
    // A site that already holds the after-await row of a handed source needs no second one.
    for defect in handed {
        seen.insert((
            defect.location.path.to_string(),
            defect.location.start_byte,
            "await".into(),
        ));
    }
    let mut emit = |site: &crate::PackageEntrySite,
                    operation: &PackageOperation<'_>,
                    plain_function: bool| {
        let Some(file) = regions.file(site.location.path.as_ref()) else {
            return;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(site.location.start_byte),
            u32::try_from(site.location.end_byte),
        ) else {
            return;
        };
        let at = Span::new(start, end);
        let contexts = regions.contexts(site.location.path.as_ref(), at);
        if contexts.none() {
            return;
        }
        // A call that enters a plain function by its own name is read by the leaf walk and the after-await walk.
        let followed = plain_function && site.kind == crate::DependencyEntry::Call;
        let via = file
            .source_text(Span::new(start, end))
            .unwrap_or_default()
            .split('(')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        let kind = match &operation.family {
            Family::Cleanup => Some(LeafOwnerOperationKind::Cleanup),
            Family::Flush => Some(LeafOwnerOperationKind::Flush),
            Family::Creation(name, _) => Some(LeafOwnerOperationKind::Primitive(name.clone())),
            Family::Read => None,
        };
        if let (Some(owner), Some(kind), false) = (&contexts.leaf, kind, followed)
            && seen.insert((
                site.location.path.to_string(),
                site.location.start_byte,
                "leaf".to_owned(),
            ))
        {
            claims.leaf_operations.push(LeafOwnerOperation {
                kind,
                owner: owner.clone(),
                location: site.location.clone(),
                fix: None,
                call_site_gate: None,
                uncertain: true,
                possible: true,
                via: Some(via.clone()),
                identity_unproven: false,
                sites: vec![operation.location.clone()],
                computation_error_unobserved: false,
            });
        }
        if contexts.apply {
            let (id, rule, message, hint, claim) = match &operation.family {
                Family::Flush if dialect.warns_inside_effect_phase(Primitive::Flush) => (
                    "SC2008",
                    "flush-in-effect-callback",
                    format!(
                        "this site runs, on an effect's apply stack, package code that calls flush() (in `{via}`'s package): the flush that drains effects is already running there, so the call drains nothing; whether the package reaches the call from here is not proven"
                    ),
                    "Call flush only from code that runs outside every effect's apply (an event handler, a timer), or remove it.",
                    true,
                ),
                Family::Creation(name, created)
                    if dialect.throws_creating_in_effect_callback(*created, false) =>
                {
                    (
                        "SC6002",
                        "primitive-in-effect-callback",
                        format!(
                            "this site runs, on an effect's apply stack, package code that calls {name}(): the apply phase has no owner, so the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK; whether the package reaches the call from here is not proven"
                        ),
                        "Create the computation in the effect's compute function or the component body, where an owner holds it.",
                        true,
                    )
                }
                _ => ("", "", String::new(), "", false),
            };
            if claim
                && seen.insert((
                    site.location.path.to_string(),
                    site.location.start_byte,
                    format!("apply:{id}"),
                ))
            {
                claims.violations.push(StaticViolation {
                    id: id.into(),
                    rule: rule.into(),
                    message,
                    hint: hint.into(),
                    location: site.location.clone(),
                    analysis_context: crate::blockers::PACKAGE_OPERATION_IN_EFFECT_APPLY.into(),
                    fixes: Vec::new(),
                    uncertain: true,
                });
            }
        }
        if contexts.after_await
            && !followed
            && matches!(operation.family, Family::Read)
            && seen.insert((
                site.location.path.to_string(),
                site.location.start_byte,
                "await".into(),
            ))
        {
            claims.defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveReadAfterAwaitThroughCall {
                    accessor: "a reactive source".into(),
                    via: via.clone(),
                },
                location: site.location.clone(),
                analysis_context: crate::blockers::PACKAGE_OPERATION_AFTER_AWAIT.into(),
                fixes: Vec::new(),
                uncertain: true,
            });
        }
    };
    for operation in operations {
        let path = operation.location.path.as_ref();
        // The innermost function written around the operation: the entry whose sites run it.
        let innermost = by_path
            .get(path)
            .into_iter()
            .flatten()
            .filter(|entry| {
                entry.function.start_byte <= operation.location.start_byte
                    && operation.location.end_byte <= entry.function.end_byte
            })
            .min_by_key(|entry| entry.function.end_byte - entry.function.start_byte);
        if let Some(entry) = innermost {
            // The call walks read the extent of a plain function (not a method, a constructor or a getter) that an
            // exact call enters, minus the functions nested in it: an operation in a getter or a closure written in
            // the entered function is not in that extent.
            let plain_function = {
                let file = operation.file;
                let (Ok(start), Ok(end), Ok(op_start), Ok(op_end)) = (
                    u32::try_from(entry.function.start_byte),
                    u32::try_from(entry.function.end_byte),
                    u32::try_from(operation.location.start_byte),
                    u32::try_from(operation.location.end_byte),
                ) else {
                    continue;
                };
                let function = Span::new(start, end);
                file.ast
                    .function_at(function)
                    .is_some_and(|function| function.method_name.is_none())
                    && crate::owners::containing_ast_function(
                        &file.ast,
                        Span::new(op_start, op_end),
                    )
                    .is_some_and(|inner| inner.span == function)
                    && !in_accessor_or_member(&file.ast, Span::new(op_start, op_end))
            };
            for site in &entry.sites {
                emit(site, operation, plain_function);
            }
        }
        // A reference that no exact call or tag decides may run any function of the package.
        let Some(root) = facts.dependency_source_root(path) else {
            continue;
        };
        for references in program
            .package_references
            .iter()
            .filter(|references| references.root == root)
        {
            for site in &references.sites {
                emit(site, operation, false);
            }
        }
    }
    claims
}

/// Whether `span` is written in the body of an object literal's getter, setter or method, or of a class's
/// constructor, method, accessor or field initializer: code that runs when a value is read, constructed or called
/// through its holder, which no walk of the call graph enters by the function's own name.
fn in_accessor_or_member(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    ast.object_properties
        .iter()
        .any(|property| !property.data && property.value.contains(span))
        || ast.classes.iter().any(|class| {
            class
                .elements
                .iter()
                .any(|element| element.value.is_some_and(|value| value.contains(span)))
        })
}

/// The family of operation a package call of `primitive` is, when it is one the leaf owner or the effect phase refuses.
fn family_of(
    dialect: &dyn solid_dialect::Dialect,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
) -> Option<Family> {
    match primitive {
        Primitive::OnCleanup => Some(Family::Cleanup),
        Primitive::Flush => Some(Family::Flush),
        created => {
            crate::cleanup::creates_forbidden_primitive(dialect, file, call, created).then(|| {
                Family::Creation(
                    file.source_text(call.callee).unwrap_or_default().to_owned(),
                    created,
                )
            })
        }
    }
}
