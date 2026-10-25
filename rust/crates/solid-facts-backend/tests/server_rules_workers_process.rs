//! The server rules (SC7005, SC7007) answer alike on one analysis worker and on
//! many.
//!
//! They run per file on the analysis workers once a project has 64 files, which
//! no fixture does. A serializer proof that is installed ends the rich-argument
//! rule at the first file in project order that asks for it, so the files
//! after it must add nothing even though the workers have already looked at
//! them. Both cases below copy a fixture among enough other modules to cross
//! that line.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::context_census_workers_process::{check, copy_directory};

fn project(name: &str, tag: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = env::temp_dir().join(format!(
        "solid-checker-server-workers-{tag}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    copy_directory(&root.join(format!("fixtures/reactive-ir/{name}")), &project);
    project
}

fn rule_locations(report: &serde_json::Value, rule: &str) -> Vec<String> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == rule)
        .map(|finding| finding["primaryLocation"].to_string())
        .collect()
}

fn write(project: &Path, name: &str, source: &str) {
    fs::write(project.join(name), source).unwrap();
}

/// No serializer: modules take the rule's branches by turn (a rich argument,
/// two violations; a JSON-safe one, silent; a spread, one obligation), and the
/// answer on every worker count is the fixture's own plus theirs.
#[test]
fn the_rich_argument_rule_answers_alike_on_every_worker_count() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = project("server-function-rich-args", "disabled");
    let fixture_findings = rule_locations(
        &check(&project, &producer, Some("1")),
        "server-function-rich-argument",
    )
    .len();
    assert_eq!(fixture_findings, 19, "the fixture's own findings");
    let modules: usize = 100;
    for index in 0..modules {
        let body = match index % 3 {
            0 => "saveEvent(new Date(), new Set<string>());",
            1 => "const payload = { title: \"plain\" }; savePlain(payload);",
            _ => "saveEvent(...(args as [Date, Set<string>]));",
        };
        write(
            &project,
            &format!("filler{index:03}.tsx"),
            &format!(
                "import {{ saveEvent, savePlain }} from \"./api\";\n\
                 declare const args: unknown;\n\
                 export function Filler{index}() {{ {body} return <i />; }}\n"
            ),
        );
    }
    let serial = check(&project, &producer, Some("1"));
    let parallel = check(&project, &producer, None);
    let found =
        |report: &serde_json::Value| rule_locations(report, "server-function-rich-argument").len();
    // Two violations for each rich module, one obligation for each spread.
    let expected = fixture_findings + 2 * modules.div_ceil(3) + (modules - 1) / 3;
    assert_eq!(found(&serial), expected, "{serial:#?}");
    assert_eq!(parallel, serial, "the whole report, not only the rule");
    fs::remove_dir_all(project).unwrap();
}

/// A serializer installed: the first file that asks for the proof ends the
/// rule, keeping what it found before asking. That file is the fixture's
/// `App.tsx`, which calls a server function with a `Date`. Modules that sort
/// before it keep their obligations, which do not ask for the proof; modules
/// that sort after it add nothing.
#[test]
fn an_installed_serializer_ends_the_rich_argument_rule_at_the_same_file() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = project("server-function-rich-args-enabled", "enabled");
    // An untyped member of a query wrapper: whether a call of it crosses the
    // transport is unproven, which is an obligation that does not ask for the
    // serializer.
    write(
        &project,
        "members.ts",
        "declare function query<A extends unknown[], R>(\n\
         \x20 fn: (...args: A) => R,\n\
         ): ((...args: A) => R) & { keyFor: any };\n\
         export const loadUser = query(async (id: number) => {\n\
         \x20 \"use server\";\n\
         \x20 return id;\n\
         });\n",
    );
    let unresolved = |name: &str, index: usize| {
        write(
            &project,
            name,
            &format!(
                "import {{ loadUser }} from \"./members\";\n\
                 export function Unresolved{index}(props: {{ when: Date }}) {{\n\
                 \x20 loadUser.keyFor(props.when);\n\
                 }}\n"
            ),
        );
    };
    let before = 40;
    for index in 0..before {
        unresolved(&format!("0before{index:02}.tsx"), index);
    }
    for index in 0..40 {
        unresolved(&format!("zafter{index:02}.tsx"), 100 + index);
    }
    let serial = check(&project, &producer, Some("1"));
    let parallel = check(&project, &producer, None);
    let kept = rule_locations(&serial, "server-function-rich-argument");
    assert_eq!(kept.len(), before, "{kept:#?}");
    assert_eq!(parallel, serial, "the whole report, not only the rule");
    fs::remove_dir_all(project).unwrap();
}

/// The response head under a `Loading` boundary: the fixture's three
/// findings, among enough other modules to run the rule on the workers, are
/// the same on every worker count, and a module's own calls are found in each.
#[test]
fn the_response_head_rule_answers_alike_on_every_worker_count() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let project = project("http-response-flush", "flush");
    let fixture_findings = rule_locations(
        &check(&project, &producer, Some("1")),
        "http-response-after-flush",
    )
    .len();
    assert_eq!(fixture_findings, 3, "the fixture's own findings");
    let source = fs::read_to_string(project.join("App.tsx")).unwrap();
    let modules = 70;
    for index in 0..modules {
        // The fixture's module again: each file is a module of its own, so
        // the same names do not meet.
        write(&project, &format!("copy{index:02}.tsx"), &source);
    }
    let serial = check(&project, &producer, Some("1"));
    let parallel = check(&project, &producer, None);
    assert_eq!(
        rule_locations(&serial, "http-response-after-flush").len(),
        fixture_findings * (modules + 1),
        "{serial:#?}"
    );
    assert_eq!(parallel, serial, "the whole report, not only the rule");
    fs::remove_dir_all(project).unwrap();
}
