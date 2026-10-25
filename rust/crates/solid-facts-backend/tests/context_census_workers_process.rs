//! The provider census (ADR 0352) answers alike on one analysis worker and on
//! many.
//!
//! The census runs per file on the analysis workers once a project has 64
//! files, which no fixture does. This copies `context-provider-closed` among
//! enough other modules to cross that line, each module taking one of the
//! census's branches, and holds the answer on the default worker count to the
//! answer on one worker and to the answer the branches imply.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn check(project: &Path, producer: &str, workers: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", producer)
        .env("SOLID_CHECKER_DAEMON", "0")
        .args(["--format", "json", "--project"])
        .arg(project.join("tsconfig.json"));
    match workers {
        Some(workers) => command.env("SOLID_CHECKER_IR_WORKERS", workers),
        None => command.env_remove("SOLID_CHECKER_IR_WORKERS"),
    };
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Modules cycle through the census's branches: a context nothing provides, one
/// a provider above its reader supplies, one stored where a holder may provide
/// it, and an exported one that another module reaches by a renamed import and
/// stores. The fixture's own `Panel` and `Memo` read a context nothing
/// provides, and its `readRemote` reads one the renamed import below hands on,
/// so that one is no longer provider-free.
#[test]
fn the_context_census_answers_alike_on_every_worker_count() {
    let Ok(producer) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = env::temp_dir().join(format!(
        "solid-checker-census-workers-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&project);
    copy_directory(
        &root.join("fixtures/reactive-ir/context-provider-closed"),
        &project,
    );
    let modules: usize = 100;
    for index in 0..modules {
        let reader = format!(
            "export function Reader{index}() {{ return <i>{{useContext(Context{index})}}</i>; }}\n"
        );
        let (declaration, tail) = match index % 4 {
            // Provider-free: one finding.
            0 => ("const", reader),
            // Provided above its reader: silent.
            1 => (
                "const",
                format!(
                    "function Reader{index}() {{ return <i>{{useContext(Context{index})}}</i>; }}\n\
                     export function Provided{index}() {{ return <Context{index} value=\"x\"><Reader{index} /></Context{index}>; }}\n"
                ),
            ),
            // Escapes into an object a holder may provide from: silent.
            2 => (
                "const",
                format!("export const held{index} = {{ context: Context{index} }};\n{reader}"),
            ),
            // Exported, and stored by another module under another name: silent.
            _ => {
                fs::write(
                    project.join(format!("holder{index}.ts")),
                    format!(
                        "import {{ Context{index} as Alias{index} }} from \"./module{index}\";\n\
                         export const held{index} = {{ context: Alias{index} }};\n"
                    ),
                )
                .unwrap();
                ("export const", reader)
            }
        };
        let declaration = if declaration == "const" {
            format!("const Context{index}")
        } else {
            format!("export const Context{index}")
        };
        fs::write(
            project.join(format!("module{index}.tsx")),
            format!(
                "import {{ createContext, useContext }} from \"solid-js\";\n\
                 {declaration} = createContext<string>();\n{tail}"
            ),
        )
        .unwrap();
    }
    // A renamed import of the fixture's exported context: a spelling no
    // candidate has, and still the same binding.
    fs::write(
        project.join("renamed.ts"),
        "import { RemoteContext as Renamed } from \"./remote\";\nexport const held = { context: Renamed };\n",
    )
    .unwrap();

    let serial = check(&project, &producer, Some("1"));
    let parallel = check(&project, &producer, None);
    let census = |report: &serde_json::Value| {
        let mut found = report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["rule"] == "missing-context-provider")
            .map(|finding| finding["primaryLocation"].to_string())
            .collect::<Vec<_>>();
        found.sort();
        found
    };
    let expected = 2 + modules.div_ceil(4);
    assert_eq!(census(&serial).len(), expected, "{:#?}", census(&serial));
    assert_eq!(census(&parallel), census(&serial));
    assert_eq!(parallel, serial, "the whole report, not only the census");
    fs::remove_dir_all(project).unwrap();
}
