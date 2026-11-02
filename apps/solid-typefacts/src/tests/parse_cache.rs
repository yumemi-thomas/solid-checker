//! The parse cache (`file_regions::ParseCache`) and the build inputs
//! (`overlay::BuildQueries`, ADR 0543): an update whose edited file changes
//! its imports builds a fresh chain, which takes every file whose text is
//! byte-equal to what it parsed before and parses again what changed, on the
//! disk as in the overlay; an in-place update that asked the disk nothing new
//! keeps the graph, and the build inputs name what the builds read.

use super::project_support::{SMALL_CONFIG, Scratch, change};
use crate::backend::{ClosureBackend, ModuleGraph};
use crate::context::Context;
use crate::project::Project;
use crate::unsafe_boundary::UpdateKind;
use typefacts::v3::ModuleGraphRequest;

fn text_of(project: &Project, path: &str) -> String {
    project
        .generation
        .as_ref()
        .unwrap()
        .program()
        .get_source_file(path)
        .unwrap_or_else(|| panic!("{path} is in the program"))
        .text()
        .to_owned()
}

fn inputs(project: &mut Project) -> typefacts::BuildInputs {
    project
        .module_graph(
            &Context::new(),
            &ModuleGraphRequest {
                build_inputs: true,
                ..ModuleGraphRequest::default()
            },
        )
        .unwrap()
        .build_inputs
        .expect("asked for")
}

#[test]
fn a_rebuilt_chain_takes_unchanged_files_and_parses_what_changed_on_disk() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "import { a } from \"./a\";\nexport const b = a;\n"),
        ("c.ts", "export const c = 2;\n"),
    ]);
    let (a, b, c) = (
        scratch.path("a.ts"),
        scratch.path("b.ts"),
        scratch.path("c.ts"),
    );
    let ctx = Context::new();
    let mut project = scratch.open();
    assert_eq!(project.parse_cache_hits(), 0);

    // An import edit: a fresh chain, which takes the files it parsed (the
    // bundled libraries, a.ts, c.ts) instead of parsing them again.
    let outcome = project
        .update(
            &ctx,
            &[change(
                &b,
                1,
                "import { a } from \"./a\";\nimport { c } from \"./c\";\nexport const b = a + c;\n",
            )],
        )
        .unwrap();
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert!(!outcome.graph_kept);
    let taken = project.parse_cache_hits();
    assert!(taken >= 2, "a.ts and c.ts were taken ({taken})");

    // a.ts edited on the disk, not through an update, then another import
    // edit: the new chain reads a.ts again and parses its new text.
    std::fs::write(&a, "export const a = 3;\nexport const extra = 4;\n").unwrap();
    project
        .update(
            &ctx,
            &[change(
                &b,
                2,
                "import { a, extra } from \"./a\";\nexport const b = a + extra;\n",
            )],
        )
        .unwrap();
    assert_eq!(project.last_update.kind, Some(UpdateKind::Rebuilt));
    assert_eq!(
        text_of(&project, &a),
        "export const a = 3;\nexport const extra = 4;\n"
    );
    assert_eq!(text_of(&project, &c), "export const c = 2;\n");

    // The answers are a fresh project's over the same files.
    std::fs::write(
        &b,
        "import { a, extra } from \"./a\";\nexport const b = a + extra;\n",
    )
    .unwrap();
    let mut fresh = crate::file_regions::without_parse_cache(|| scratch.open());
    assert_eq!(fresh.parse_cache_hits(), 0);
    assert_eq!(
        project.source_digests(&ctx).unwrap(),
        fresh.source_digests(&ctx).unwrap()
    );
    let graph = |project: &mut Project| {
        let mut inventory = project
            .module_graph(
                &ctx,
                &ModuleGraphRequest {
                    imports: true,
                    ..ModuleGraphRequest::default()
                },
            )
            .unwrap();
        inventory.build_inputs = None;
        inventory
    };
    assert_eq!(graph(&mut project), graph(&mut fresh));
}

#[test]
fn an_in_place_update_keeps_the_graph_and_the_build_inputs_name_what_was_read() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
        ("b.ts", "import { a } from \"./a\";\nexport const b = a;\n"),
    ]);
    let (a, b) = (scratch.path("a.ts"), scratch.path("b.ts"));
    let mut project = scratch.open();
    let opened = inputs(&mut project);
    assert!(opened.reads.iter().any(|path| **path == *a), "{opened:?}");
    assert!(opened.reads.iter().any(|path| **path == *b), "{opened:?}");
    assert!(
        opened.files.iter().chain(&opened.directories).count() > 0,
        "the resolution probes are named: {opened:?}"
    );
    assert!(
        opened.reads.iter().all(|path| !path.contains("://")),
        "a bundled library is not on the disk"
    );

    // Same imports: replaced in place, nothing new asked.
    let outcome = project
        .update(
            &Context::new(),
            &[change(
                &b,
                1,
                "import { a } from \"./a\";\nexport const b = a + 1;\n",
            )],
        )
        .unwrap();
    assert_eq!(project.last_update.kind, Some(UpdateKind::Reused));
    assert!(outcome.graph_kept);
    assert_eq!(inputs(&mut project), opened);

    // A new import of a file the program does not hold: a fresh chain, whose
    // inputs are its own build's and name the new file.
    std::fs::write(scratch.path("d.ts"), "export const d = 1;\n").unwrap();
    let outcome = project
        .update(
            &Context::new(),
            &[change(
                &b,
                2,
                "import { d } from \"./d\";\nexport const b = d;\n",
            )],
        )
        .unwrap();
    assert!(!outcome.graph_kept);
    let rebuilt = inputs(&mut project);
    assert!(
        rebuilt
            .reads
            .iter()
            .any(|path| **path == *scratch.path("d.ts"))
    );
}
