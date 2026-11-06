//! ADR 0659: which build runs a site. A hydrated app runs a server pass and a
//! client pass, a server-only project runs one, and `if (isServer)` hands a site
//! to one of them. A rule that claims what the client runtime does holds only
//! where the client runs the site, so these facts answer once, over every
//! finding, at the end of projection.

use serde::{Deserialize, Serialize};
use solid_facts::core::Span;

use crate::{Finding, RuntimeEnvironment, RuntimeRendering, RuntimeTarget};

/// Whether a client build runs this program.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientPath {
    #[default]
    Runs,
    /// The selection names a server build and no client root exists.
    Absent,
    /// Only server entries are visible and nothing was selected: a client
    /// entry may sit outside the project.
    Unresolved,
}

/// Which side a guarded region belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmSide {
    /// Under a proven `isServer`: a client build never runs it.
    ServerOnly,
    /// Under a build constant no selection decides here (`typeof window`,
    /// `isDev`, `process.env`, `import.meta.env`).
    BuildConstant,
    /// The body of a component `clientOnly` loads, or `lazy` where a server pass
    /// exists. Its first mount goes through a gate memo or resumes served
    /// content, a later one calls it in a window, so no read claim holds here
    /// (ADR 0664). A child's own body has its window back.
    DeferredMount,
    /// `<NoHydration>`'s children where a hydrating pass runs: `NoHydration`
    /// returns `undefined` while hydrating, and a non-hydrating mount renders
    /// them, so no client claim holds here either way (ADR 0677).
    NoHydration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildArm {
    pub path: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub side: ArmSide,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPaths {
    #[serde(default)]
    pub client: ClientPath,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arms: Vec<BuildArm>,
}

impl ExecutionPaths {
    fn arm_at(&self, finding: &Finding) -> Option<ArmSide> {
        let site = &finding.primary_location;
        let inside = |arm: &&BuildArm| {
            arm.path == site.path.as_ref()
                && arm.start_byte <= site.start_byte
                && site.end_byte <= arm.end_byte
        };
        let arms = || {
            self.arms
                .iter()
                .filter(inside)
                .filter(|arm| arm.side != ArmSide::DeferredMount)
        };
        arms()
            .find(|arm| arm.side == ArmSide::ServerOnly)
            .or_else(|| arms().next())
            .map(|arm| arm.side)
    }

    /// ADR 0664: a read claim in a body whose mount form is open.
    pub(crate) fn apply_deferred_mount(&self, finding: &mut Finding) {
        let site = &finding.primary_location;
        if finding.kind == "violation"
            && matches!(finding.id.as_str(), "SC1001" | "SC5001" | "SC5003")
            && self.arms.iter().any(|arm| {
                arm.side == ArmSide::DeferredMount
                    && arm.path == site.path.as_ref()
                    && arm.start_byte <= site.start_byte
                    && site.end_byte <= arm.end_byte
            })
        {
            finding.kind = "uncertifiable".into();
            finding.message = format!(
                "this body is mounted by a loader whose first mount and later mounts differ in whether a read window is open, so this claim is not proven either way; if a window is open: {}",
                finding.message
            );
            crate::blockers::MOUNT_FORM_WINDOW.clone_into(&mut finding.analysis_context);
            finding.fixes.clear();
        }
    }

    /// Answers one finding of a rule that claims the client runtime: `false`
    /// drops it, proved silent. A server build has no strict window and no
    /// write guard (`docs/open-work.md`, the server build table).
    pub(crate) fn apply(&self, finding: &mut Finding) -> bool {
        let arm = self.arm_at(finding);
        if self.client == ClientPath::Absent || arm == Some(ArmSide::ServerOnly) {
            return false;
        }
        let blocker = if self.client == ClientPath::Unresolved {
            crate::blockers::CLIENT_ROOT_UNPROVEN
        } else if arm == Some(ArmSide::BuildConstant) {
            crate::blockers::BUILD_CONSTANT_ARM
        } else if arm == Some(ArmSide::NoHydration) {
            crate::blockers::NO_HYDRATION_SUBTREE
        } else {
            return true;
        };
        if finding.kind == "violation" {
            finding.kind = "uncertifiable".into();
            finding.message = format!(
                "{} This claims the client build, and whether one runs this site is not proven.",
                finding.message
            );
            blocker.clone_into(&mut finding.analysis_context);
            finding.fixes.clear();
        }
        true
    }
}

/// Whether any file reaches a client root: `render` or `hydrate`, from Solid or
/// a testing library. By imported name on purpose: this keeps findings, so it
/// over-approximates, and only its absence proves anything. An import no
/// value reference uses roots nothing.
pub(crate) fn client_root_present(
    facts: &solid_facts::ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
) -> bool {
    const ROOTS: [&str; 2] = ["render", "hydrate"];
    facts.files.iter().any(|file| {
        file.ast.imports.iter().any(|import| {
            !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding.runtime_referenced
                        && (binding
                            .imported
                            .as_deref()
                            .is_some_and(|imported| ROOTS.contains(&imported))
                            || (binding.kind == solid_facts::ast::ImportKind::Namespace
                                && dialect.owns_module(import.module.as_str())
                                && file.ast.members.iter().any(|member| {
                                    file.source_text(member.object)
                                        == file.source_text(binding.local.span)
                                        && file
                                            .source_text(member.property)
                                            .is_some_and(|name| ROOTS.contains(&name))
                                })))
                })
        })
    })
}

pub(crate) fn execution_paths(
    facts: &solid_facts::ProjectFacts,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
    lookup: &crate::indexes::SemanticLookup<'_>,
) -> ExecutionPaths {
    let client = client_path(facts, runtime, dialect);
    let files = &facts.files;
    let hydrates = hydrating_pass_present(files, runtime, dialect);
    let mut arms = Vec::new();
    for file in files {
        file_arms(file, dialect, &mut arms);
        deferred_mount_arms(facts, file, runtime, dialect, &mut arms);
        if hydrates {
            no_hydration_arms(file, lookup, &mut arms);
        }
    }
    arm_only_functions(files, runtime, lookup, &mut arms);
    ExecutionPaths { client, arms }
}

/// Whether a hydrating pass may run: the selection says so, or a file calls
/// `hydrate`. Without one, `NoHydration` renders its children and every claim
/// in them stands.
fn hydrating_pass_present(
    files: &[solid_facts::FileFacts],
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
) -> bool {
    matches!(runtime.rendering, Some(RuntimeRendering::Hydrated))
        || files.iter().any(|file| {
            file.ast
                .calls
                .iter()
                .any(|call| named_import(file, dialect, call.callee, "hydrate"))
        })
}

/// ADR 0677: the elements `NoHydration` wraps, `Hydration` inside them included:
/// the outer one returned nothing, so the inner one never ran on the client.
fn no_hydration_arms(
    file: &solid_facts::FileFacts,
    lookup: &crate::indexes::SemanticLookup<'_>,
    arms: &mut Vec<BuildArm>,
) {
    for element in &file.ast.jsx_elements {
        if crate::jsx_tag_is(lookup, file, element, "NoHydration") {
            arms.push(BuildArm {
                path: file.path.as_str().to_owned(),
                start_byte: u64::from(element.span.start),
                end_byte: u64::from(element.span.end),
                side: ArmSide::NoHydration,
            });
        }
    }
}

/// How a reference reaches a function: called, or as a JSX tag or a value.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Reach {
    Call,
    Other,
}

/// ADR 0677: a function every reference of which lies in an arm runs only there,
/// so a site in it is in the arm. A helper an arm calls is the case arms alone
/// missed (ADR 0659's limit), and a helper a deferred body calls is the same one
/// (ADR 0664's). Runs to a fixpoint: arms only grow, so it ends. A function with
/// no symbol, or no reference, stays out, which leaves its finding where it was.
fn arm_only_functions(
    files: &[solid_facts::FileFacts],
    runtime: &RuntimeEnvironment,
    lookup: &crate::indexes::SemanticLookup<'_>,
    arms: &mut Vec<BuildArm>,
) {
    if arms.is_empty() {
        return;
    }
    let by_path = |path: &str| files.iter().find(|file| file.path.as_str() == path);
    grow_to_fixpoint(
        arms,
        |arm| {
            by_path(&arm.path).map_or_else(Vec::new, |file| {
                functions_named_in(file, arm, lookup)
                    .into_iter()
                    .map(|(target, function)| {
                        (
                            (target.path.as_str().to_owned(), function.span),
                            (target, function),
                        )
                    })
                    .collect()
            })
        },
        |&(target, function), arms| {
            let side = arm_of_every_reference(target, function, runtime, lookup, files, arms)?;
            Some(BuildArm {
                path: target.path.as_str().to_owned(),
                start_byte: u64::from(function.span.start),
                end_byte: u64::from(function.span.end),
                side,
            })
        },
    );
}

/// Grows `arms` until no candidate joins. An arm's candidates are read once,
/// when it joins; each undecided one is asked again every round, because an arm
/// that joined may settle it, and never once it has joined.
fn grow_to_fixpoint<A, K: Clone + Eq + std::hash::Hash, C>(
    arms: &mut Vec<A>,
    mut named: impl FnMut(&A) -> Vec<(K, C)>,
    mut joins: impl FnMut(&C, &[A]) -> Option<A>,
) {
    let mut known = std::collections::HashSet::new();
    let mut undecided: Vec<C> = Vec::new();
    let mut read = 0;
    loop {
        for arm in &arms[read..] {
            for (key, candidate) in named(arm) {
                if known.insert(key) {
                    undecided.push(candidate);
                }
            }
        }
        read = arms.len();
        let mut grown = Vec::new();
        undecided.retain(|candidate| match joins(candidate, arms) {
            Some(arm) => {
                grown.push(arm);
                false
            }
            None => true,
        });
        if grown.is_empty() {
            return;
        }
        arms.extend(grown);
    }
}

/// The functions an arm names by a call, a JSX tag or an attribute or argument.
fn functions_named_in<'l>(
    file: &solid_facts::FileFacts,
    arm: &BuildArm,
    lookup: &crate::indexes::SemanticLookup<'l>,
) -> Vec<(
    &'l solid_facts::FileFacts,
    &'l solid_facts::ast::FunctionFact,
)> {
    let inside =
        |span: Span| arm.start_byte <= u64::from(span.start) && u64::from(span.end) <= arm.end_byte;
    let path = file.path.as_str();
    let mut spans = Vec::new();
    for call in file.ast.calls.iter().filter(|call| inside(call.span)) {
        spans.push(call.callee);
        spans.extend(call.arguments.iter().map(|argument| argument.span));
    }
    for element in file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| inside(element.span))
    {
        spans.push(element.name.span);
        spans.extend(
            element
                .attributes
                .iter()
                .filter_map(|attribute| attribute.expression),
        );
    }
    spans
        .into_iter()
        .filter_map(|span| lookup.function_called_at(path, file.ast.peel_ts_sugar_span(span)))
        .collect()
}

/// The arm every reference of `function` lies in, or `None` when one lies
/// elsewhere. `DeferredMount` passes only a call: a tag's body has its window
/// back. A server-only arm reached by a function other files may import proves
/// nothing about those callers, so that answer is open, not silent.
fn arm_of_every_reference(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    runtime: &RuntimeEnvironment,
    lookup: &crate::indexes::SemanticLookup<'_>,
    files: &[solid_facts::FileFacts],
    arms: &[BuildArm],
) -> Option<ArmSide> {
    let symbol = lookup.symbol_of_function(file, function)?;
    let references = lookup.symbol_references(symbol.as_str());
    let name = function
        .name
        .as_ref()
        .or_else(|| crate::owners::function_binding_name(file, function))?;
    let mut sides = Vec::new();
    for reference in &references {
        let start = u32::try_from(reference.start_byte).ok()?;
        let end = u32::try_from(reference.end_byte).ok()?;
        let span = Span::new(start, end);
        if *reference.path == *file.path.as_str()
            && (function.span.contains(span) || name.span == span)
        {
            continue;
        }
        let target = files
            .iter()
            .find(|target| target.path.as_str() == &*reference.path)?;
        if module_specifier_reference(target, span) {
            continue;
        }
        let reach = if target
            .ast
            .calls
            .iter()
            .any(|call| target.ast.peel_ts_sugar_span(call.callee) == span)
        {
            Reach::Call
        } else {
            Reach::Other
        };
        let side = arms
            .iter()
            .filter(|arm| {
                arm.path == *reference.path
                    && arm.start_byte <= reference.start_byte
                    && reference.end_byte <= arm.end_byte
                    && (arm.side != ArmSide::DeferredMount || reach == Reach::Call)
            })
            .map(|arm| arm.side)
            .min_by_key(|side| side_rank(*side))?;
        sides.push(side);
    }
    let side = sides.into_iter().max_by_key(|side| side_rank(*side))?;
    let open_to_importers =
        !runtime.program_is_closed() && crate::indexes::binding_exported(file, name.span);
    Some(if side == ArmSide::ServerOnly && open_to_importers {
        ArmSide::BuildConstant
    } else {
        side
    })
}

/// Whether `span` is an import or export specifier, which names a binding and
/// runs nothing. Not `export.span`: `export function f` holds all of `f`.
pub(crate) fn module_specifier_reference(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast
        .imports
        .iter()
        .any(|import| import.span.contains(span))
        || file
            .ast
            .exports
            .iter()
            .flat_map(|export| export.specifiers.iter())
            .any(|specifier| specifier.local.span == span)
}

/// Which arm answers when a function sits in several: the most decided first.
const fn side_rank(side: ArmSide) -> u8 {
    match side {
        ArmSide::ServerOnly => 0,
        ArmSide::NoHydration => 1,
        ArmSide::DeferredMount => 2,
        ArmSide::BuildConstant => 3,
    }
}

/// ADR 0664: the bodies a client-only or resumed loader mounts. A module named
/// only through a shape this does not read gives the whole file: the mount is
/// open, so nothing in it is claimed.
fn deferred_mount_arms(
    facts: &solid_facts::ProjectFacts,
    file: &solid_facts::FileFacts,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
    arms: &mut Vec<BuildArm>,
) {
    let server_pass = runtime.selects_server_build()
        || matches!(runtime.rendering, Some(RuntimeRendering::Hydrated));
    for load in &file.ast.module_loads {
        let Some(specifier) = load.specifier.as_deref() else {
            continue;
        };
        let loaded = file.ast.arguments_containing(load.span).any(|(call, _)| {
            named_import(file, dialect, call.callee, "clientOnly")
                || (server_pass && named_import(file, dialect, call.callee, "lazy"))
        });
        if !loaded {
            continue;
        }
        let Some(path) = solid_facts::resolve_relative_module_path(
            file.path.as_str(),
            specifier,
            facts.files.iter().map(|target| target.path.as_str()),
        ) else {
            continue;
        };
        let Some(target) = facts
            .files
            .iter()
            .find(|target| target.path.as_str() == path)
        else {
            continue;
        };
        let body = target
            .ast
            .exports
            .iter()
            .find(|export| {
                export.kind == solid_facts::ast::ExportKind::Default
                    && export.module.is_none()
                    && target.ast.functions_within(export.span).next().is_some()
            })
            .map(|export| (u64::from(export.span.start), u64::from(export.span.end)));
        let (start_byte, end_byte) = body.unwrap_or((0, u64::MAX));
        arms.push(BuildArm {
            path: path.to_owned(),
            start_byte,
            end_byte,
            side: ArmSide::DeferredMount,
        });
    }
}

fn client_path(
    facts: &solid_facts::ProjectFacts,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
) -> ClientPath {
    if matches!(
        runtime.rendering,
        Some(RuntimeRendering::Hydrated | RuntimeRendering::Csr)
    ) || runtime.target == Some(RuntimeTarget::Browser)
    {
        return ClientPath::Runs;
    }
    let root = client_root_present(facts, dialect);
    if runtime.selects_server_build() {
        return if root {
            ClientPath::Runs
        } else {
            ClientPath::Absent
        };
    }
    if !root && crate::source_discovery::project_server_rendering(facts, runtime, dialect).renders()
    {
        return ClientPath::Unresolved;
    }
    ClientPath::Runs
}

/// What a test says about the build.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Test {
    /// `true`: truthy on the server only. `false`: truthy on the client only.
    Server(bool),
    Build,
}

const MAX_ALIAS: usize = 4;

fn file_arms(
    file: &solid_facts::FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    arms: &mut Vec<BuildArm>,
) {
    let ast = &file.ast;
    let mut push = |span: Span, side: ArmSide| {
        arms.push(BuildArm {
            path: file.path.as_str().to_owned(),
            start_byte: u64::from(span.start),
            end_byte: u64::from(span.end),
            side,
        });
    };
    let branches = |test: Span,
                    truthy: Option<Span>,
                    falsy: Option<Span>,
                    push: &mut dyn FnMut(Span, ArmSide)| {
        match classify(file, dialect, test, MAX_ALIAS) {
            Some(Test::Server(true)) => {
                if let Some(arm) = truthy {
                    push(arm, ArmSide::ServerOnly);
                }
            }
            Some(Test::Server(false)) => {
                if let Some(arm) = falsy {
                    push(arm, ArmSide::ServerOnly);
                }
            }
            Some(Test::Build) => {
                for arm in [truthy, falsy].into_iter().flatten() {
                    push(arm, ArmSide::BuildConstant);
                }
            }
            None => {}
        }
    };
    for region in &ast.if_regions {
        branches(
            region.test,
            Some(region.consequent),
            region.alternate,
            &mut push,
        );
        // `if (!isServer) return;`: the rest of the function is the other side.
        let leaves = region.alternate.is_none()
            && ast.returns.iter().any(|returned| {
                region.consequent.contains(returned.span)
                    && returned.control_tests.as_ref() == [region.test]
            });
        if leaves
            && let Some(function) = crate::owners::containing_ast_function(ast, region.consequent)
        {
            let rest = Span::new(region.consequent.end, function.body.end);
            branches(region.test, None, Some(rest), &mut push);
        }
    }
    for conditional in &ast.conditional_expressions {
        branches(
            conditional.test,
            Some(conditional.consequent),
            Some(conditional.alternate),
            &mut push,
        );
    }
    for logical in &ast.logical_expressions {
        match logical.operator {
            solid_facts::ast::LogicalOperatorKind::And => {
                branches(logical.left, Some(logical.right), None, &mut push);
            }
            solid_facts::ast::LogicalOperatorKind::Or => {
                branches(logical.left, None, Some(logical.right), &mut push);
            }
            _ => {}
        }
    }
}

fn classify(
    file: &solid_facts::FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    span: Span,
    depth: usize,
) -> Option<Test> {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let text = file.source_text(span)?.trim();
    let invert = |test: Option<Test>| {
        test.map(|test| match test {
            Test::Server(truthy) => Test::Server(!truthy),
            Test::Build => Test::Build,
        })
    };
    if let Some(inner) = text.strip_prefix('!') {
        let offset = u32::try_from(text.len() - inner.len()).ok()?;
        return invert(classify(
            file,
            dialect,
            Span::new(span.start + offset, span.end),
            depth,
        ));
    }
    if let Some(logical) = ast
        .logical_expressions
        .iter()
        .find(|logical| logical.span == span)
    {
        let left = classify(file, dialect, logical.left, depth);
        let right = classify(file, dialect, logical.right, depth);
        return match logical.operator {
            solid_facts::ast::LogicalOperatorKind::And => [left, right]
                .into_iter()
                .flatten()
                .find(|test| *test == Test::Server(true))
                .or_else(|| left.or(right).filter(|test| *test == Test::Build)),
            _ => left.or(right).filter(|test| *test == Test::Build),
        };
    }
    for (suffix, same) in [
        ("=== true", true),
        ("!== false", true),
        ("== true", true),
        ("=== false", false),
        ("!== true", false),
        ("== false", false),
    ] {
        if let Some(left) = text.strip_suffix(suffix) {
            let length = u32::try_from(left.trim_end().len()).ok()?;
            let test = classify(
                file,
                dialect,
                Span::new(span.start, span.start + length),
                depth,
            );
            return if same { test } else { invert(test) };
        }
    }
    if server_export(file, dialect, span) {
        return Some(Test::Server(true));
    }
    if [
        "typeof window",
        "typeof document",
        "import.meta.env",
        "process.env",
    ]
    .iter()
    .any(|constant| text.contains(constant))
        || named_import(file, dialect, span, "isDev")
    {
        return Some(Test::Build);
    }
    if depth == 0 {
        return None;
    }
    let declaration = ast.reference_declaration(span)?;
    let binding = ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
    let initializer = binding
        .initializer_identifier
        .as_ref()
        .map(|identifier| identifier.span)
        .or(binding.initializer)
        .filter(|_| binding.immutable)?;
    classify(file, dialect, initializer, depth - 1)
}

fn server_export(
    file: &solid_facts::FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    span: Span,
) -> bool {
    dialect
        .host_boolean_exports()
        .iter()
        .filter(|export| export.server && !export.client)
        .any(|export| named_import(file, dialect, span, export.export))
}

/// `span` is exactly an import of `name` from a module the dialect owns, or a
/// member `name` of such a namespace.
pub(crate) fn named_import(
    file: &solid_facts::FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    span: Span,
    name: &str,
) -> bool {
    let ast = &file.ast;
    let bound = |local: Span, namespace: bool| {
        ast.imports.iter().any(|import| {
            dialect.owns_module(import.module.as_str())
                && import.bindings.iter().any(|binding| {
                    binding.local.span == local
                        && if namespace {
                            binding.kind == solid_facts::ast::ImportKind::Namespace
                        } else {
                            binding.imported.as_deref() == Some(name)
                        }
                })
        })
    };
    if let Some(member) = ast.member_at(span) {
        return file.source_text(member.property) == Some(name)
            && ast
                .reference_declaration(ast.peel_ts_sugar_span(member.object))
                .is_some_and(|declaration| bound(declaration, true));
    }
    ast.reference_declaration(span)
        .is_some_and(|declaration| bound(declaration, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop this replaced, as the reference: every arm's names, every round.
    fn plain_fixpoint(
        arms: &mut Vec<u32>,
        named: &dyn Fn(&u32) -> Vec<u32>,
        joins: &dyn Fn(&u32, &[u32]) -> Option<u32>,
    ) {
        let mut done = std::collections::HashSet::new();
        loop {
            let mut grown = Vec::new();
            for arm in arms.iter() {
                for candidate in named(arm) {
                    if done.contains(&candidate) {
                        continue;
                    }
                    if let Some(joined) = joins(&candidate, arms) {
                        done.insert(candidate);
                        grown.push(joined);
                    }
                }
            }
            if grown.is_empty() {
                return;
            }
            arms.extend(grown);
        }
    }

    #[test]
    fn the_fixpoint_reads_each_arm_once_and_ends_where_the_plain_loop_ends() {
        // An arm names the next few numbers. A candidate joins once its
        // `needs` is an arm, so one that fails a round must be asked again.
        let named = |arm: &u32| {
            (1..=3)
                .map(|step| arm + step)
                .filter(|n| *n < 40)
                .collect::<Vec<_>>()
        };
        for shape in 2..9_u32 {
            let joins = move |candidate: &u32, arms: &[u32]| {
                let needs = candidate.saturating_sub(shape);
                (arms.contains(&needs) && !candidate.is_multiple_of(7)).then_some(*candidate)
            };
            let mut plain = vec![0];
            plain_fixpoint(&mut plain, &named, &joins);

            let mut reads = 0;
            let mut grown = vec![0];
            grow_to_fixpoint(
                &mut grown,
                |arm| {
                    reads += 1;
                    named(arm).into_iter().map(|n| (n, n)).collect()
                },
                |candidate, arms| joins(candidate, arms),
            );
            assert_eq!(grown, plain, "shape {shape}");
            assert_eq!(reads, grown.len(), "each arm is read once");
        }
    }

    #[test]
    fn a_candidate_a_later_arm_settles_is_asked_again() {
        // 2 waits for 1, and 1 joins only in the first round.
        let mut arms = vec![0];
        grow_to_fixpoint(
            &mut arms,
            |_| vec![(2, 2), (1, 1)],
            |candidate, arms| {
                (candidate - 1 == 0 || arms.contains(&(candidate - 1))).then_some(*candidate)
            },
        );
        assert_eq!(arms, [0, 1, 2]);
    }

    fn finding(kind: &str, start: u64) -> Finding {
        let mut finding: Finding = serde_json::from_value(serde_json::json!({
            "id": "SC1001",
            "rule": "strict-read-untracked",
            "kind": kind,
            "message": "m",
            "severity": "warning",
            "primaryLocation": {
                "path": "a.tsx", "startByte": start, "endByte": start + 1
            },
            "relatedLocations": []
        }))
        .unwrap();
        finding.hint = "h".into();
        finding
    }

    fn arm(side: ArmSide) -> BuildArm {
        BuildArm {
            path: "a.tsx".into(),
            start_byte: 10,
            end_byte: 20,
            side,
        }
    }

    #[test]
    fn a_client_claim_holds_only_where_a_client_runs_the_site() {
        let paths = |client, arms| ExecutionPaths { client, arms };
        let runs = paths(ClientPath::Runs, vec![]);
        let mut row = finding("violation", 12);
        assert!(runs.apply(&mut row));
        assert_eq!(row.kind, "violation");

        assert!(!paths(ClientPath::Absent, vec![]).apply(&mut finding("violation", 12)));
        assert!(!paths(ClientPath::Absent, vec![]).apply(&mut finding("uncertifiable", 12)));

        let server_arm = paths(ClientPath::Runs, vec![arm(ArmSide::ServerOnly)]);
        assert!(!server_arm.apply(&mut finding("violation", 12)));
        let mut outside = finding("violation", 30);
        assert!(server_arm.apply(&mut outside));
        assert_eq!(outside.kind, "violation");

        let mut constant = finding("violation", 12);
        assert!(paths(ClientPath::Runs, vec![arm(ArmSide::BuildConstant)]).apply(&mut constant));
        assert_eq!(constant.kind, "uncertifiable");
        assert_eq!(constant.analysis_context, "build-constant-arm");

        let mut unresolved = finding("violation", 12);
        assert!(paths(ClientPath::Unresolved, vec![]).apply(&mut unresolved));
        assert_eq!(unresolved.kind, "uncertifiable");
        assert_eq!(unresolved.analysis_context, "client-root-unproven");
    }

    #[test]
    fn a_server_arm_beats_a_build_constant_around_it() {
        let paths = ExecutionPaths {
            client: ClientPath::Runs,
            arms: vec![arm(ArmSide::BuildConstant), arm(ArmSide::ServerOnly)],
        };
        assert!(!paths.apply(&mut finding("violation", 12)));
    }

    #[test]
    fn hydrated_selects_both_paths_and_refuses_node() {
        let hydrated = RuntimeEnvironment {
            rendering: Some(RuntimeRendering::Hydrated),
            ..RuntimeEnvironment::default()
        };
        assert!(hydrated.validate().is_ok());
        assert!(!hydrated.selects_server_build());
        let conditions = hydrated.selected_conditions();
        assert!(conditions.contains("hydrated") && conditions.contains("browser"));
        let node = RuntimeEnvironment {
            target: Some(RuntimeTarget::Node),
            ..hydrated.clone()
        };
        assert!(node.validate().is_err());
        let browser = RuntimeEnvironment {
            target: Some(RuntimeTarget::Browser),
            ..hydrated
        };
        assert!(browser.validate().is_ok());
        assert!(
            crate::source_discovery::ServerRenderingPremise::select(
                Some(RuntimeRendering::Hydrated),
                false
            )
            .renders()
        );
    }
}
