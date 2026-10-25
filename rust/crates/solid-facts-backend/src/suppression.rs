//! Suppression comments (ADR 0387): which findings an `eslint-disable` or
//! `solid-checker-disable` comment removes, and the notice for a name that
//! resolves to nothing.
//!
//! solid-facts records only the syntax of a comment (label, names, position).
//! What a name denotes is decided here, through the dialect catalog, the
//! former-name table and the group table (ADR 0386), because those are the
//! backend's. The matching rules are ESLint's (`apply-disable-directives.js`),
//! so a comment means the same to the checker as to ESLint.
//!
//! A comment removes **violations at a site** and nothing else. An
//! uncertifiable result is an obligation, not a defect the author can accept,
//! and a project notice has no site; neither is ever suppressed, and a comment
//! naming a rule whose results are all uncertifiable suppresses nothing.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use solid_facts::ProjectFacts;
use solid_facts::ast::{
    AstFacts, LineTable, SuppressionAction, SuppressionCommentFact, SuppressionOrigin,
};
use solid_facts::core::Span;
use solid_reactive_ir::Finding;

use crate::dialect::{self, Dialect};

/// What one name in a comment can reach.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Matcher {
    Rule(String),
    /// A former name that covered one code of a merged rule (ADR 0384).
    RuleCode {
        rule: String,
        code: String,
    },
    Code(String),
}

impl Matcher {
    fn reaches(&self, finding: &Finding) -> bool {
        match self {
            Self::Rule(rule) => finding.rule == *rule,
            Self::RuleCode { rule, code } => finding.rule == *rule && finding.id == *code,
            Self::Code(code) => finding.id == *code,
        }
    }
}

/// The rules a comment names: every rule when it names none.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Reach {
    All,
    Named(Vec<Matcher>),
}

impl Reach {
    fn reaches(&self, finding: &Finding) -> bool {
        match self {
            Self::All => true,
            Self::Named(matchers) => matchers.iter().any(|matcher| matcher.reaches(finding)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Resolution {
    Matchers(Vec<Matcher>),
    /// Accepted and meaningless here: another plugin's name, an upstream rule
    /// this checker has no rule for, an adapter id, a retired identity.
    Inert,
    /// A `solid-checker/*` name nothing resolves: SC9016.
    Unknown,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Namespace {
    Checker,
    Upstream,
}

/// What a name as written in a comment denotes.
///
/// `solid-checker/<name>` and `solid/<name>` are the two plugin namespaces; a
/// `solid-checker-*` comment also takes the bare name. In an `eslint-*` comment
/// a bare or foreign-prefixed name belongs to ESLint or to another plugin
/// (`prefer-rest-params`, `@typescript-eslint/no-unused-vars`) and is not read.
fn resolve(dialect: &Dialect, origin: SuppressionOrigin, written: &str) -> Resolution {
    let (namespace, name) = if let Some(name) = written.strip_prefix("solid-checker/") {
        (Namespace::Checker, name)
    } else if let Some(name) = written.strip_prefix("solid/") {
        (Namespace::Upstream, name)
    } else if origin == SuppressionOrigin::SolidChecker {
        (Namespace::Checker, written)
    } else {
        return Resolution::Inert;
    };
    match namespace {
        Namespace::Checker => {
            checker_name(dialect, name, origin == SuppressionOrigin::SolidChecker)
        }
        // `solid/*` is eslint-plugin-solid's namespace: only its pinned names
        // are ours to read, and one with no checker rule says nothing here.
        Namespace::Upstream if dialect::UPSTREAM_RULE_NAMES.contains(&name) => {
            match checker_name(dialect, name, false) {
                Resolution::Unknown => Resolution::Inert,
                other => other,
            }
        }
        Namespace::Upstream => Resolution::Inert,
    }
}

fn checker_name(dialect: &Dialect, name: &str, codes: bool) -> Resolution {
    if dialect::ADAPTER_RULE_IDS.contains(&name) {
        return Resolution::Inert;
    }
    if let Some(members) = dialect::rule_group(name) {
        return Resolution::Matchers(
            members
                .iter()
                .map(|member| Matcher::Rule((*member).to_owned()))
                .collect(),
        );
    }
    let canonical = dialect::rule_alias(name).unwrap_or(name);
    if (dialect.has_rule)(canonical) {
        return Resolution::Matchers(vec![match dialect::rule_alias_code(name) {
            Some(code) => Matcher::RuleCode {
                rule: canonical.to_owned(),
                code: code.to_owned(),
            },
            None => Matcher::Rule(canonical.to_owned()),
        }]);
    }
    if codes && (dialect.has_code)(name) {
        // An obligation's code (SC9005, SC9012) resolves, and reaches nothing:
        // its results are never site violations (ADR 0398).
        return Resolution::Matchers(vec![Matcher::Code(name.to_owned())]);
    }
    if codes && dialect::retired_code(name) {
        return Resolution::Inert;
    }
    // A former or retired name is a known identity with nothing left to
    // suppress: a retired rule cannot fire, and a merged one is deliberately
    // not carried over (ADR 0384).
    if dialect::rule_alias(name).is_some() || dialect::retired_rule(name).is_some() {
        return Resolution::Inert;
    }
    Resolution::Unknown
}

/// One origin's comments, in the form ESLint applies them: block comments are
/// ordered switches by position, line comments cover one line.
#[derive(Debug, Default)]
struct Scan {
    /// `(offset, disable, reach)`, by offset.
    block: Vec<(u32, bool, Reach)>,
    /// `(line, reach)`.
    lines: Vec<(u32, Reach)>,
}

impl Scan {
    fn suppresses(&self, finding: &Finding, offset: u32, line: u32) -> bool {
        if self
            .lines
            .iter()
            .any(|(covered, reach)| *covered == line && reach.reaches(finding))
        {
            return true;
        }
        // A disable switches the rule off from its position on and an enable
        // switches it back on; the last one at or before the finding decides.
        let mut disabled = false;
        for (position, disable, reach) in &self.block {
            if *position > offset {
                break;
            }
            if reach.reaches(finding) {
                disabled = *disable;
            }
        }
        disabled
    }
}

/// The two comment families scan independently, as ESLint's own directives and
/// a comment ESLint does not read do.
#[derive(Debug)]
struct FileDirectives {
    lines: LineTable,
    eslint: Scan,
    checker: Scan,
}

impl FileDirectives {
    fn suppresses(&self, finding: &Finding) -> bool {
        let offset = u32::try_from(finding.primary_location.start_byte).unwrap_or(u32::MAX);
        let line = self.lines.line(offset);
        self.eslint.suppresses(finding, offset, line)
            || self.checker.suppresses(finding, offset, line)
    }
}

/// A name no catalog entry resolves, and where it was written.
struct UnknownName {
    name: String,
    path: String,
    span: Span,
}

fn compile(
    dialect: &Dialect,
    host: &BTreeSet<String>,
    path: &str,
    source: &str,
    ast: &AstFacts,
    unknown: &mut Vec<UnknownName>,
) -> Option<FileDirectives> {
    if ast.suppression_comments.is_empty() {
        return None;
    }
    let lines = LineTable::new(source);
    let mut directives = FileDirectives {
        lines,
        eslint: Scan::default(),
        checker: Scan::default(),
    };
    for comment in &ast.suppression_comments {
        let reach = reach_of(dialect, host, path, comment, unknown);
        let scan = match comment.origin {
            SuppressionOrigin::Eslint => &mut directives.eslint,
            SuppressionOrigin::SolidChecker => &mut directives.checker,
        };
        let Some(reach) = reach else { continue };
        // The last character of the comment decides its last line.
        let first = directives.lines.line(comment.span.start);
        let last = directives.lines.line(comment.span.end.saturating_sub(1));
        match comment.action {
            SuppressionAction::Disable => scan.block.push((comment.span.start, true, reach)),
            SuppressionAction::Enable => scan.block.push((comment.span.start, false, reach)),
            // ESLint refuses a `-line` comment that spans lines.
            SuppressionAction::DisableLine if first == last => scan.lines.push((first, reach)),
            SuppressionAction::DisableLine => {}
            SuppressionAction::DisableNextLine => scan.lines.push((last + 1, reach)),
        }
    }
    Some(directives)
}

/// What a comment reaches, or `None` when it reaches nothing.
fn reach_of(
    dialect: &Dialect,
    host: &BTreeSet<String>,
    path: &str,
    comment: &SuppressionCommentFact,
    unknown: &mut Vec<UnknownName>,
) -> Option<Reach> {
    if comment.names.is_empty() {
        return Some(Reach::All);
    }
    let mut matchers = Vec::new();
    for written in &comment.names {
        // ESLint suppresses a message by the rule id it was reported under, so
        // a name it applies itself is not applied twice.
        if comment.origin == SuppressionOrigin::Eslint
            && written
                .strip_prefix("solid-checker/")
                .is_some_and(|rule| host.contains(rule))
        {
            continue;
        }
        match resolve(dialect, comment.origin, written) {
            Resolution::Matchers(found) => matchers.extend(found),
            Resolution::Inert => {}
            Resolution::Unknown => unknown.push(UnknownName {
                name: written.to_string(),
                path: path.to_owned(),
                span: comment.span,
            }),
        }
    }
    (!matchers.is_empty()).then_some(Reach::Named(matchers))
}

/// Removes the violations a suppression comment covers, and returns the
/// notices for the names nothing resolves.
///
/// Run after every reclassification of a violation into an uncertifiable
/// result, so a comment can only ever remove a violation that would have been
/// reported.
pub(crate) fn apply(
    dialect: &Dialect,
    facts: &ProjectFacts,
    host: &BTreeSet<String>,
    findings: &mut Vec<Finding>,
) -> Vec<Finding> {
    let mut unknown = Vec::new();
    let mut by_path: HashMap<&str, FileDirectives> = HashMap::new();
    for file in &facts.files {
        // A dependency's own comments are not the project's.
        if facts.dependency_source_root(file.path.as_str()).is_some() {
            continue;
        }
        if let Some(directives) = compile(
            dialect,
            host,
            file.path.as_str(),
            &file.source,
            &file.ast,
            &mut unknown,
        ) {
            by_path.insert(file.path.as_str(), directives);
        }
    }
    retain_unsuppressed(&by_path, findings);
    unknown_name_findings(&unknown)
}

/// Drops the site violations a comment covers. Everything else stays: an
/// uncertifiable result has no author-acceptable defect to accept, and a
/// project-scoped finding has no site.
fn retain_unsuppressed(by_path: &HashMap<&str, FileDirectives>, findings: &mut Vec<Finding>) {
    if by_path.is_empty() {
        return;
    }
    findings.retain(|finding| {
        !(finding.kind == "violation"
            && finding.subject_kind != "project"
            && by_path
                .get(finding.primary_location.path.as_ref())
                .is_some_and(|directives| directives.suppresses(finding)))
    });
}

/// One notice per distinct name, at its first use, with the other uses as
/// related locations.
fn unknown_name_findings(unknown: &[UnknownName]) -> Vec<Finding> {
    let mut sites: BTreeMap<&str, Vec<(&str, Span)>> = BTreeMap::new();
    for entry in unknown {
        sites
            .entry(&entry.name)
            .or_default()
            .push((&entry.path, entry.span));
    }
    sites
        .into_iter()
        .map(|(name, mut uses)| {
            uses.sort();
            uses.dedup();
            unknown_suppression_finding(name, &uses)
        })
        .collect()
}

fn unknown_suppression_finding(name: &str, uses: &[(&str, Span)]) -> Finding {
    let location = |(path, span): &(&str, Span)| span.location(*path);
    let metadata = solid_reactive_ir::RuleMetadata {
        code: dialect::UNKNOWN_SUPPRESSION_CODE,
        name: dialect::UNKNOWN_SUPPRESSION_RULE,
        severity: "warning",
        uncertifiable: true,
        default_enabled: true,
        configurable: true,
        presets: &[],
    };
    let mut finding = Finding::new(
        metadata,
        format!(
            "a suppression comment names {name:?}, which is not a rule of this checker; it suppresses nothing for that name"
        ),
        location(&uses[0]),
    );
    finding.hint = "Name a rule from docs/rules, one of its former names, the `reactivity` group, or (in a `solid-checker-*` comment) an SC code. A name in another plugin's namespace is not read, and `eslint-disable` takes names separated by commas.".into();
    finding.analysis_context = "suppression-comments".into();
    finding.subject_kind = "project".into();
    finding.related_locations = uses[1..].iter().map(location).collect();
    finding
}

#[cfg(test)]
mod tests;
