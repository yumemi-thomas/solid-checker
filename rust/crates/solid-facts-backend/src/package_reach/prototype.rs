//! Whether an installed package's closure runs code the build does not see (the second re-check of #232): a file of the closure names `Object.prototype` or builds code as a
//! string (`solid_reactive_ir::unseen_code_in`, the rule the analysis asks of the files of the program), or the closure is not read to its end.
//!
//! A package that gives a shared prototype an accessor (`Object.defineProperty(Object.prototype, "h", { get() { … this.m() … } })`) reads any key of any plain object through
//! it, wherever the program reads one, and nothing in the project shows the read; a package the build cannot read may do the same, and unavailable source is no evidence
//! (AGENTS.md, invariants 2 and 3). The action walk of [`super::package_reach`] answers another question and stops at the first hazard and at the first load of a Solid
//! package, so this is a walk of its own over the same closure: it follows every load the action walk follows, the Solid packages' own (the dialect's, audited) aside, and
//! it ends in an answer only when every file was read.
//!
//! What ends the closure undecided: a file or load the walk cannot read (a missing file, a native module, a file type it does not read, a closure past the limit, an
//! `exports` map that names no file), and a file whose scan finds a load by a computed name, code generation or a file Oxc does not parse. A global the pinned libraries do
//! not declare and the global object handed on hide no code (the second is the rule's own question of the file's text), so they do not end it.

//! Whether an installed package's closure runs code the build does not see (the second re-check of #232): a file of the closure names `Object.prototype` or builds code as a
//! string (`solid_reactive_ir::unseen_code_in`, the rule the analysis asks of the files of the program), or the closure is not read to its end.
//!
//! A package that gives a shared prototype an accessor (`Object.defineProperty(Object.prototype, "h", { get() { … this.m() … } })`) reads any key of any plain object through
//! it, wherever the program reads one, and nothing in the project shows the read; a package the build cannot read may do the same, and unavailable source is no evidence
//! (AGENTS.md, invariants 2 and 3). The answer comes from the one walk of the closure that [`super::package_reach`] makes (ADR 0974): the walk reads every file once, so what
//! each file says here (the rule's answer on its facts, [`routes_in`]) is a step of the file's visit beside the hazards and loads, and the closure's `why` is the first place in
//! walk order that a hazard hides code, the walk cannot read a file or a load, or a file names `Object.prototype` or builds code. It follows every load the action walk follows, the
//! Solid packages' own (the dialect's, audited) aside, and it ends in an answer only when every file was read.
//!
//! What ends the closure undecided: a file or load the walk cannot read (a missing file, a native module, a file type it does not read, a closure past the limit, an
//! `exports` map that names no file), and a file whose scan finds a load by a computed name, code generation or a file Oxc does not parse. A global the pinned libraries do
//! not declare and the global object handed on hide no code (the second is the rule's own question of the file's text), so they do not end it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
use solid_facts::core::{Generation, SourceHash};
use solid_reactive_ir::UnseenCode;

/// What a package's closure was found to run that the build does not see.
#[derive(Clone, Debug, Default)]
pub(super) struct Closure {
    /// Why the closure runs code the build does not see, in a phrase; `None` where every file of it was read and none does.
    pub(super) why: Option<String>,
    /// Whether any file of the closure holds a `this` or a `super`.
    pub(super) reads_receiver: bool,
}

/// A file's answer, kept for the process while its length and modification time hold: a package several closures load is asked once.
type Kept = Mutex<HashMap<PathBuf, ((u64, Option<SystemTime>), Result<UnseenCode, String>)>>;

/// Whether the file names `Object.prototype` or builds code as a string, by the rule the analysis asks of the files of the program ([`solid_reactive_ir::unseen_code_in`]), on
/// the file's own facts. `Err` where the file does not parse.
pub(super) fn routes_in(file: &Path) -> Result<UnseenCode, String> {
    static KEPT: OnceLock<Kept> = OnceLock::new();
    let metadata = fs::metadata(file).map_err(|error| error.to_string())?;
    let stamp = (metadata.len(), metadata.modified().ok());
    let kept = KEPT.get_or_init(Mutex::default);
    if let Some((kept_stamp, answer)) = kept
        .lock()
        .map_err(|_| "answer cache poisoned".to_owned())?
        .get(file)
        && *kept_stamp == stamp
    {
        return answer.clone();
    }
    let answer = ask(file);
    kept.lock()
        .map_err(|_| "answer cache poisoned".to_owned())?
        .insert(file.to_path_buf(), (stamp, answer.clone()));
    answer
}

fn ask(file: &Path) -> Result<UnseenCode, String> {
    let source = fs::read_to_string(file).map_err(|error| error.to_string())?;
    let ast = solid_facts::ast::extract(file.to_string_lossy().into_owned(), &source)
        .map_err(|error| error.to_string())?;
    // The rule reads the file's syntax; nothing of the compiler's execution map is read, so it is empty.
    let compiler = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(&source),
        semantic_model: Default::default(),
        tracked_regions: Vec::new(),
        untracked_regions: Vec::new(),
        discarded_regions: Vec::new(),
        ownership_regions: Vec::new(),
        callback_roles: Vec::new(),
        jsx_operations: Vec::new(),
    };
    let generation = Generation::new(1).map_err(|error| error.to_string())?;
    let facts = solid_facts::FileFacts::new(generation, source.as_str(), ast, compiler)
        .map_err(|error| error.to_string())?;
    Ok(solid_reactive_ir::unseen_code_in(&facts))
}

#[cfg(test)]
mod tests {
    use super::super::{Demand, Memo, SolidPackages, package_reach};
    use super::*;
    use std::collections::BTreeSet;

    const SOLID: SolidPackages<'static> = SolidPackages {
        names: &["solid-js", "@solidjs/signals", "@solidjs/web"],
        scopes: &["@solidjs/"],
    };

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    struct Project(PathBuf);

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A project holding `solid-js` and the packages given as `(name, [(file, text)])`; each package's manifest names `index.js` as its main.
    fn project(packages: &[(&str, &[(&str, &str)])]) -> Project {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-package-prototype-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        write(
            &directory,
            "node_modules/solid-js/package.json",
            r#"{"name":"solid-js","version":"2.0.0"}"#,
        );
        write(
            &directory,
            "node_modules/solid-js/index.js",
            "export const action = (f) => f;",
        );
        for (name, files) in packages {
            write(
                &directory,
                &format!("node_modules/{name}/package.json"),
                &format!(r#"{{"name":"{name}","version":"1.0.0","main":"index.js"}}"#),
            );
            for (file, text) in *files {
                write(&directory, &format!("node_modules/{name}/{file}"), text);
            }
        }
        Project(directory)
    }

    fn closure_of(project: &Project, name: &str) -> Closure {
        let root = project
            .0
            .join("node_modules")
            .join(name)
            .canonicalize()
            .unwrap();
        // A package the program loads (`.`): only such a closure is asked.
        let demand = Demand {
            subpaths: BTreeSet::from([".".to_owned()]),
            everything: true,
        };
        package_reach(
            &root,
            &SOLID,
            &demand,
            &BTreeSet::new(),
            &mut Memo::default(),
        )
        .unseen
    }

    #[test]
    fn a_package_that_names_object_prototype_runs_code_the_build_does_not_see() {
        let routes = [
            (
                "object-define",
                "Object.defineProperty(Object.prototype, 'h', { get() { return 1; } });",
            ),
            (
                "object-getproto",
                "Object.getPrototypeOf({}).__defineGetter__('h', function () { return 1; });",
            ),
            (
                "global-this",
                "Object.defineProperty(globalThis.Object.prototype, 'h', { get() { return 1; } });",
            ),
            (
                "exported-prototype",
                "export const proto = Object.prototype;",
            ),
            (
                "a-function-never-called",
                "export function setup() { Object.defineProperty(Object.prototype, 'h', {}); }",
            ),
            (
                "code-it-builds",
                "export const put = (el, markup) => { el.innerHTML = markup; };",
            ),
        ];
        for (name, text) in routes {
            let project = project(&[(name, &[("index.js", text)])]);
            let found = closure_of(&project, name);
            assert!(found.why.is_some(), "{name}: {found:?}");
        }
    }

    #[test]
    fn a_package_that_names_nothing_of_the_kind_is_seen() {
        let project = project(&[
            (
                "pure",
                &[(
                    "index.js",
                    "export const add = (a, b) => a + b; export const keys = (o) => Object.keys(o);",
                )],
            ),
            (
                "receiver",
                &[("index.js", "export class A { get x() { return this.y; } }")],
            ),
        ]);
        let pure = closure_of(&project, "pure");
        assert!(pure.why.is_none() && !pure.reads_receiver, "{pure:?}");
        let receiver = closure_of(&project, "receiver");
        assert!(
            receiver.why.is_none() && receiver.reads_receiver,
            "{receiver:?}"
        );
    }

    #[test]
    fn the_whole_closure_is_walked_past_a_load_of_a_solid_package_and_through_other_packages() {
        let project = project(&[
            (
                "after-solid",
                &[
                    ("index.js", "import 'solid-js'; import './later.js';"),
                    (
                        "later.js",
                        "Object.defineProperty(Object.prototype, 'h', {});",
                    ),
                ],
            ),
            ("outer", &[("index.js", "import 'inner';")]),
            (
                "inner",
                &[(
                    "index.js",
                    "Object.defineProperty(Object.prototype, 'h', {});",
                )],
            ),
        ]);
        assert!(closure_of(&project, "after-solid").why.is_some());
        assert!(closure_of(&project, "outer").why.is_some());
    }

    #[test]
    fn a_closure_that_is_not_read_to_its_end_is_undecided() {
        let project = project(&[
            (
                "computed",
                &[("index.js", "export const load = (name) => import(name);")],
            ),
            ("missing", &[("index.js", "import './nowhere.js';")]),
            (
                "generated",
                &[("index.js", "export const run = (code) => eval(code);")],
            ),
            (
                "native",
                &[("index.js", "import './addon.node';"), ("addon.node", "")],
            ),
        ]);
        for name in ["computed", "missing", "generated", "native"] {
            let found = closure_of(&project, name);
            assert!(
                found
                    .why
                    .as_deref()
                    .is_some_and(|why| why.starts_with("undecided: ")),
                "{name}: {found:?}"
            );
        }
    }

    #[test]
    fn a_solid_package_is_the_dialects_and_is_not_asked() {
        let project = project(&[]);
        let found = closure_of(&project, "solid-js");
        assert!(found.why.is_none() && !found.reads_receiver, "{found:?}");
    }
}
