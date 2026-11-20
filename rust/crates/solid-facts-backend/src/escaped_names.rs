//! ADR 1152: a program that writes a name with an escape is not proven.
//!
//! A name is its decoded value: `eval` written `ev\u0061l` is `eval`, and a string `"cl\u0069ck"` is the event `click`. Many checks compare a name to a literal, take its first
//! letter (is this a component?) or look it up in a list, and read the source text of its span, which for such a name is the spelling. A check that fails to see a name is wrong
//! in both directions: a violation is reported at code that is not one (a component the first-letter test no longer takes for one, a global `Promise` no longer seen), and a
//! violation is lost (a registration, an `eval`, an option key no longer seen). The first is a false positive, which no report may hold; the second is a silent clean, which the
//! proof of a clean must never rest on. Neither is proven, so nothing in the program is.
//!
//! The corpus writes no identifier with an escape in any of its 459,755 source files, and a string that is one word (a name some check could compare) with an escape in 17
//! dependency files of them (the census of `scripts/fuzz/escape-census.mjs`), so the answer is a veto, not a decoding: when any file of the program writes a name with an escape
//! (the facts `escaped_names` and `escaped_strings`: an identifier, or a string or template whose whole value is a word; a plain string such as `"\n"` is none), then
//!
//! - every violation the program's rules reported is uncertifiable, whatever the file it stands in (a file's names reach the others through the summaries, the call graph and
//!   the component test), under the named blocker [`ESCAPED_NAME_UNRESOLVED`]; and
//! - every file of the project gets one uncertifiable finding spanning the whole file, under the same blocker, so no site of it reads clean: the sites a misread name hid from
//!   the analysis have no finding to demote, and the file cannot be certified.
//!
//! Neither is shown to a user (an uncertifiable finding of subject `file` is internal, ADR 0328). The style rules (SC80xx) claim nothing about execution and a proven compiler
//! refusal (SC9015) is the compiler's, not the checker's reading of a name: they stay.

use solid_facts::{FileFacts, ProjectFacts};
use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, blocked_context};
use typefacts::Location;

/// The named blocker of every finding this veto demotes or adds.
pub(crate) const ESCAPED_NAME_UNRESOLVED: &str = "escaped-name-unresolved";

/// The rule the veto's notices are of: SC9012, the obligation that carries a named blocker for what the analysis could not see (ADR 0440). Every catalog declares it.
pub(crate) const OBLIGATION_RULE: &str = "reactive-dispatch-unresolved";

/// The first name a file writes with an escape: where, as written, and the name it is.
struct Escaped<'f> {
    file: &'f FileFacts,
    start: u32,
    end: u32,
    name: String,
}

impl Escaped<'_> {
    fn raw(&self) -> &str {
        self.file
            .source
            .get(self.start as usize..self.end as usize)
            .unwrap_or_default()
    }

    fn location(&self) -> Location {
        Location {
            path: self.file.path.as_str().into(),
            start_byte: u64::from(self.start),
            end_byte: u64::from(self.end),
        }
    }
}

/// The first name `file` writes with an escape, by position; `None` when it writes none.
fn first_escape(file: &FileFacts) -> Option<Escaped<'_>> {
    let identifier = file
        .ast
        .escaped_names
        .first()
        .map(|fact| (fact.span.start, fact.span.end, fact.name.to_string()));
    let string = file
        .ast
        .escaped_strings
        .first()
        .map(|fact| (fact.span.start, fact.span.end, fact.text.to_string()));
    let (start, end, name) = match (identifier, string) {
        (Some(left), Some(right)) => {
            if (left.0, left.1) <= (right.0, right.1) {
                left
            } else {
                right
            }
        }
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return None,
    };
    Some(Escaped {
        file,
        start,
        end,
        name,
    })
}

/// The files of the program that write a name with an escape, each with its first.
fn escaping_files(facts: &ProjectFacts) -> Vec<Escaped<'_>> {
    facts.files.iter().filter_map(first_escape).collect()
}

/// Whether a finding is one a misread name can make wrong: a violation of a rule that claims something about execution. A style rule (SC80xx) is about syntax, and a proven
/// compiler refusal is the compiler's.
fn demotable(finding: &Finding) -> bool {
    finding.kind == "violation"
        && !finding.id.starts_with("SC80")
        && finding.id != crate::dialect::COMPILER_REFUSED_CODE
}

/// What the veto did: how many violations it demoted and how many files it noticed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Veto {
    pub(crate) demoted: usize,
    pub(crate) noticed: usize,
}

/// Applies the veto to `findings`: demotes every violation, adds the notice of every project file. `metadata` is the catalog's identity of [`OBLIGATION_RULE`]; a catalog
/// without it still has its violations demoted, and has no notice to add. Nothing changes for a program none of whose files writes a name with an escape.
pub(crate) fn veto(
    facts: &ProjectFacts,
    metadata: Option<RuleMetadata>,
    findings: &mut Vec<Finding>,
) -> Veto {
    let escaping = escaping_files(facts);
    if escaping.is_empty() {
        return Veto::default();
    }
    let mut report = Veto::default();
    for finding in findings.iter_mut().filter(|finding| demotable(finding)) {
        finding.analysis_context =
            blocked_context(ESCAPED_NAME_UNRESOLVED, &finding.analysis_context);
        finding.kind = "uncertifiable".into();
        finding.message.push_str(
            "; the program writes a name with an escape, which is not read as the name it denotes, so this is not proven",
        );
        report.demoted += 1;
    }
    let Some(metadata) = metadata else {
        return report;
    };
    let evidence = evidence_of(&escaping);
    for file in &facts.files {
        if facts.dependency_source_root(file.path.as_str()).is_some() {
            continue;
        }
        findings.push(notice(metadata, file, &escaping, evidence.clone()));
        report.noticed += 1;
    }
    report
}

/// The evidence of a notice: the first name each of the first few files writes with an escape.
fn evidence_of(escaping: &[Escaped<'_>]) -> Vec<EvidenceStep> {
    const SHOWN: usize = 3;
    let mut steps = escaping
        .iter()
        .take(SHOWN)
        .map(|escaped| EvidenceStep {
            message: format!(
                "{} writes the name `{}` as `{}`",
                escaped.file.path.as_str(),
                escaped.name,
                escaped.raw()
            ),
            location: Some(escaped.location()),
        })
        .collect::<Vec<_>>();
    if escaping.len() > SHOWN {
        steps.push(EvidenceStep {
            message: format!("and {} more files", escaping.len() - SHOWN),
            location: None,
        });
    }
    steps
}

/// The notice of `file`: it spans the whole file, so every site of it stands under it.
fn notice(
    metadata: RuleMetadata,
    file: &FileFacts,
    escaping: &[Escaped<'_>],
    evidence: Vec<EvidenceStep>,
) -> Finding {
    let location = Location {
        path: file.path.as_str().into(),
        start_byte: 0,
        end_byte: u64::try_from(file.source.len()).unwrap_or(u64::MAX),
    };
    let first = &escaping[0];
    let mut finding = Finding::new(
        metadata,
        format!(
            "{} writes the name `{}` as `{}`, which is not read as the name it denotes, so nothing in {} is proven",
            first.file.path.as_str(),
            first.name,
            first.raw(),
            file.path.as_str(),
        ),
        location,
    );
    finding.hint = "Write the name without an escape (an identifier, or a string that is one word) for the checker to prove anything about the program.".into();
    finding.analysis_context = blocked_context(ESCAPED_NAME_UNRESOLVED, "");
    finding.subject_kind = "file".into();
    finding.evidence = evidence;
    finding
}

#[cfg(test)]
mod tests;
