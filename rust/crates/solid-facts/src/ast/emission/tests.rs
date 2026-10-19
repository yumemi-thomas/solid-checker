use super::{
    EmittingStatement, MODULE_LADDER, ModuleEmission, ModuleEmissionError, ModuleFlavor,
    ParseFlavor, has_export_assignment, module_emission, module_emission_with_ladder,
};
use serde::Deserialize;

#[test]
fn export_assignment_is_the_equals_form_at_module_level_only() {
    for (source, expected) in [
        (
            "declare namespace React { const x: 1 }\nexport = React;\n",
            true,
        ),
        (
            "declare const value: number;\nexport default value;\n",
            false,
        ),
        ("export declare const IS_DEV: boolean;\n", false),
        // Inside an ambient module block it assigns that module's export,
        // not this file's.
        (
            "declare module \"other\" { const x: 1; export = x; }\nexport {};\n",
            false,
        ),
    ] {
        assert_eq!(has_export_assignment(source), Ok(expected), "{source}");
    }
    assert!(has_export_assignment("export = ;").is_err());
}

/// The shared corpus. Both this module's tests and
/// `packages/cli/test/artifact-resolution.test.mjs` read these exact bytes
/// and these exact verdicts, so a divergence between the TypeScript and Oxc
/// statement tables is a test failure here rather than a whole-proposal
/// refusal in the field.
const CASES: &str = include_str!("../../../../../../fixtures/module-emission/cases.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    format: String,
    cases_version: u16,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    source: String,
    module: Verdict,
    declaration_file: Verdict,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Verdict {
    NonEmitting,
    Empty,
    NonDeclaring,
    Unparsable,
    Emitting(String),
    /// Not non-emitting, with the exact variant deliberately unpinned. Used
    /// only where Oxc's and TypeScript's *grammars* disagree about whether
    /// the bytes parse at all — a `.d.ts` carrying an implementation body is
    /// a parse error to one and a gated statement to the other. Both refuse,
    /// which is the load-bearing half; a case may only use this verdict when
    /// no answer either side can give is `non-emitting`.
    Refused,
}

fn corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(CASES).expect("shared corpus parses");
    assert_eq!(corpus.format, "solid-checker-module-emission-cases");
    assert_eq!(corpus.cases_version, 1);
    corpus
}

fn rendered(answer: &Result<ModuleEmission, ModuleEmissionError>) -> String {
    match answer {
        Ok(ModuleEmission::NonEmitting) => "non-emitting".into(),
        Ok(ModuleEmission::Empty) => "empty".into(),
        Ok(ModuleEmission::NonDeclaring) => "non-declaring".into(),
        Ok(ModuleEmission::Emitting(statement)) => format!("emitting:{}", statement.kind),
        Err(_) => "unparsable".into(),
    }
}

fn matches(verdict: &Verdict, answer: &str) -> bool {
    match verdict {
        Verdict::Refused => answer != "non-emitting",
        other => expected(other) == answer,
    }
}

fn expected(verdict: &Verdict) -> String {
    match verdict {
        Verdict::NonEmitting => "non-emitting".into(),
        Verdict::Empty => "empty".into(),
        Verdict::NonDeclaring => "non-declaring".into(),
        Verdict::Unparsable => "unparsable".into(),
        Verdict::Emitting(kind) => format!("emitting:{kind}"),
        Verdict::Refused => "refused".into(),
    }
}

#[test]
fn the_shared_corpus_answers_both_premises_case_for_case() {
    let corpus = corpus();
    assert!(corpus.cases.len() >= 100, "the corpus is the attack list");
    let mut mismatches = Vec::new();
    for case in &corpus.cases {
        for (flavor, verdict) in [
            (ModuleFlavor::Module, &case.module),
            (ModuleFlavor::DeclarationFile, &case.declaration_file),
        ] {
            let answer = rendered(&module_emission(&case.source, flavor));
            if !matches(verdict, &answer) {
                mismatches.push(format!(
                    "{} under {flavor:?}: answered {answer}, corpus says {}",
                    case.name,
                    expected(verdict)
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} corpus mismatch(es):\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

/// The parse ladder's order must never decide an answer. Any mutation that
/// makes it matter — a configuration that parses the same bytes into a
/// different statement list, or a table that reads the source type — fails
/// here.
#[test]
fn every_parse_order_agrees_on_every_case() {
    let corpus = corpus();
    let orders = [
        [
            ParseFlavor::Ts,
            ParseFlavor::DeclarationTs,
            ParseFlavor::Tsx,
        ],
        [
            ParseFlavor::Ts,
            ParseFlavor::Tsx,
            ParseFlavor::DeclarationTs,
        ],
        [
            ParseFlavor::DeclarationTs,
            ParseFlavor::Ts,
            ParseFlavor::Tsx,
        ],
        [
            ParseFlavor::DeclarationTs,
            ParseFlavor::Tsx,
            ParseFlavor::Ts,
        ],
        [
            ParseFlavor::Tsx,
            ParseFlavor::Ts,
            ParseFlavor::DeclarationTs,
        ],
        [
            ParseFlavor::Tsx,
            ParseFlavor::DeclarationTs,
            ParseFlavor::Ts,
        ],
    ];
    assert_eq!(
        orders[0], MODULE_LADDER,
        "the default order is one of these"
    );
    for case in &corpus.cases {
        let baseline = rendered(&module_emission(&case.source, ModuleFlavor::Module));
        for order in &orders {
            assert_eq!(
                rendered(&module_emission_with_ladder(&case.source, order)),
                baseline,
                "{} under {order:?}",
                case.name
            );
        }
    }
}

#[test]
fn the_first_emitting_statement_is_reported_with_its_byte_range() {
    let source = "export type T = 1;\nexport const value = 2;\nstart();";
    assert_eq!(
        module_emission(source, ModuleFlavor::Module),
        Ok(ModuleEmission::Emitting(EmittingStatement {
            kind: "variable declaration",
            start: 19,
            end: 42,
        }))
    );
}

#[test]
fn identical_bytes_get_one_answer_whatever_the_publisher_calls_the_file() {
    // The plain-module premise is blind to the filename by construction: it
    // is handed bytes and nothing else. This is the assertion that would
    // have to be deleted for a suffix guess to creep back into it.
    let ambient = "export declare function createRenderer(options: unknown): unknown;";
    assert_eq!(
        module_emission(ambient, ModuleFlavor::Module),
        Ok(ModuleEmission::NonEmitting)
    );
    assert_eq!(
        module_emission(&format!("{ambient}\n"), ModuleFlavor::Module),
        Ok(ModuleEmission::NonEmitting)
    );
}

/// The gate that keeps the declaration-file premise from being a bare
/// suffix guess: bytes TypeScript would refuse in an ambient context are
/// not a declaration file, whatever the member is called.
///
/// Oxc's grammar already refuses a *top-level* ambient implementation body
/// or initializer outright (`declare const value = 1;` does not parse at
/// all), so those shapes never reach the gate here — the shared corpus
/// records them as refused-without-a-pinned-variant, because TypeScript
/// accepts them and reaches the mirror gate instead. What reaches this gate
/// is everything nested inside an ambient block, where both grammars are
/// permissive.
#[test]
fn the_ambient_gate_names_the_shape_that_breaks_the_suffix_claim() {
    for (source, kind) in [
        (
            "declare module \"m\" { export const value: number; start(); }",
            "expression statement",
        ),
        (
            "declare module \"m\" { import \"./effects.js\"; export const value: number; }",
            "side-effect import",
        ),
        (
            "declare module \"m\" { export const value = 1; }",
            "variable initializer",
        ),
        (
            "declare namespace N { export function f() { return 1; } }",
            "implementation body",
        ),
        (
            "declare global { function f(): void { return; } }",
            "implementation body",
        ),
        ("declare class C { static { } }", "class static block"),
        (
            "declare class C { accessor value = 1; }",
            "property initializer",
        ),
    ] {
        let answer = module_emission(source, ModuleFlavor::DeclarationFile);
        assert_eq!(rendered(&answer), format!("emitting:{kind}"), "{source:?}");
    }
}

/// Every gated shape is refused under the premise that admits the suffix.
/// This is the assertion the mirror in the generator must preserve: a claim
/// it makes that this premise would refuse costs the whole proposal.
///
/// Under the bytes-only premise several of these *are* non-emitting, and
/// deliberately so — `declare const value = 1;` is a TS1039 error that still
/// emits nothing. Which premise a member gets is decided by its suffix by
/// the caller, and exactly one premise runs, so the two answers never race:
/// see `artifactCaseDisposition` and
/// `ArtifactSnapshot::prove_non_emitting_module_target`.
#[test]
fn no_gated_shape_is_ever_non_emitting_as_a_declaration_file() {
    for source in [
        "declare const value = 1;",
        "declare class C { m() { return 1; } }",
        "declare class C { value = 1; }",
        "export declare function f(): void { return; }",
        "declare module \"m\" { export const value = 1; }",
        "declare namespace N { export function f() { return 1; } }",
        "declare class C { static { } }",
    ] {
        assert_ne!(
            rendered(&module_emission(source, ModuleFlavor::DeclarationFile)),
            "non-emitting",
            "{source:?}"
        );
    }
}
