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
        let arms = || self.arms.iter().filter(inside);
        arms()
            .find(|arm| arm.side == ArmSide::ServerOnly)
            .or_else(|| arms().next())
            .map(|arm| arm.side)
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
) -> ExecutionPaths {
    let client = client_path(facts, runtime, dialect);
    let mut arms = Vec::new();
    for file in &facts.files {
        file_arms(file, dialect, &mut arms);
    }
    ExecutionPaths { client, arms }
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
