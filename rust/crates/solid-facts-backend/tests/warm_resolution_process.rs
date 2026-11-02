//! A retained daemon after the disk moves what an import resolves to (ADR
//! 0544): a `package.json` edit, a new file that shadows a resolution, the
//! file removed again, a `paths` change and a symlink swapped to another
//! target, each between two checks of one directory, and import edits that
//! make the producer build a fresh program chain from files it parsed before
//! (`file_regions::ParseCache`). Every answer is compared, byte for byte with
//! its exit status, with the one-shot answer at the same path, and each move
//! between the two versions of `readCount` must change the answer, so a
//! retained resolution or parse that missed the move shows.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::process_support::temporary_directory;

const FEEDBACK_ARGS: [&str; 9] = [
    "--format",
    "json",
    "--feedback-facts",
    "--runtime-target",
    "browser",
    "--runtime-build",
    "development",
    "--rendering",
    "csr",
];

/// `readCount` that reads the signal (`Bad` reads it untracked) and one that
/// does not, for a module `depth` directories below the project root.
fn reactive(depth: usize) -> String {
    let up = if depth == 0 {
        "./".to_owned()
    } else {
        "../".repeat(depth)
    };
    format!(
        "import {{ count }} from \"{up}source\";\nexport function readCount() {{\n  return count();\n}}\n"
    )
}

const PLAIN: &str = "export function readCount() {\n  return 1;\n}\n";

fn app(specifier: &str, extra: &str) -> String {
    format!(
        "import {{ invoke }} from \"./source\";\nimport {{ readCount }} from \"{specifier}\";\n{extra}\n\
         export function Good() {{\n  return <div>{{invoke(readCount)}}</div>;\n}}\n\n\
         export function Bad() {{\n  const value = invoke(readCount);\n  return <div>{{value}}</div>;\n}}\n"
    )
}

fn tsconfig(paths: &str) -> String {
    format!(
        "{{\n  \"compilerOptions\": {{\n    \"jsx\": \"preserve\",\n    \"module\": \"ESNext\",\n    \
         \"moduleResolution\": \"Bundler\",\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \
         \"paths\": {{ \"@dep\": [\"{paths}\"] }}\n  }},\n  \"include\": [\"**/*.ts\", \"**/*.tsx\"]\n}}\n"
    )
}

/// One project directory checked again and again, as a fuzzer slot is.
struct Warm {
    root: PathBuf,
    sockets: PathBuf,
    typefacts: String,
}

impl Warm {
    fn new(label: &str) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = temporary_directory(label).canonicalize().unwrap();
        let sockets = PathBuf::from(format!("/tmp/sdq-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sockets);
        fs::create_dir_all(&sockets).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/callback-forwarding");
        for name in ["solid-js.d.ts", "source.ts"] {
            fs::copy(fixture.join(name), root.join(name)).unwrap();
        }
        Some(Self {
            root,
            sockets,
            typefacts,
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.path(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn remove(&self, name: &str) {
        fs::remove_file(self.path(name)).unwrap();
    }

    fn check(&self, daemon: bool) -> (Option<i32>, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &self.typefacts)
            .env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" })
            .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
            .env("SOLID_CHECKER_TIMINGS", "1")
            .env("SOLID_CHECKER_IR_SHADOW", "0")
            .env("TMPDIR", &self.sockets)
            .current_dir(&self.root)
            .arg("--project")
            .arg(self.path("tsconfig.json"))
            .args(FEEDBACK_ARGS)
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The retained answer, which must be the one-shot answer at this path.
    fn agree(&self, step: &str) -> String {
        let (status, warm, stderr) = self.check(true);
        let (once_status, once, once_stderr) = self.check(false);
        assert!(matches!(once_status, Some(0 | 1)), "{step}: {once_stderr}");
        assert!(
            stderr.contains("\"mode\":\"retained-daemon\""),
            "{step}: not a retained answer: {stderr}"
        );
        assert_eq!(status, once_status, "{step}: {stderr}");
        assert_eq!(warm, once, "{step}");
        warm
    }

    fn daemon_pids(&self) -> Vec<String> {
        let output = Command::new("pgrep")
            .args(["-f", "--"])
            .arg(format!(
                "--serve --project {}",
                self.path("tsconfig.json").display()
            ))
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for Warm {
    fn drop(&mut self) {
        for pid in self.daemon_pids() {
            let _ = Command::new("kill").arg(pid).status();
        }
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.sockets);
    }
}

#[cfg(unix)]
fn symlink(target: &str, link: &Path) {
    let _ = fs::remove_file(link);
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn a_retained_daemon_follows_every_disk_change_that_moves_a_resolution() {
    let Some(warm) = Warm::new("warm-resolution") else {
        return;
    };
    // `./dep` is a directory whose package.json names its entry.
    warm.write("tsconfig.json", &tsconfig("./paths-a.ts"));
    warm.write("dep/package.json", "{ \"types\": \"./reactive.ts\" }\n");
    warm.write("dep/reactive.ts", &reactive(1));
    warm.write("dep/plain.ts", PLAIN);
    warm.write("paths-a.ts", &reactive(0));
    warm.write("paths-b.ts", PLAIN);
    warm.write("variants/reactive/index.ts", &reactive(2));
    warm.write("variants/plain/index.ts", PLAIN);
    warm.write("App.tsx", &app("./dep", ""));
    let reads = warm.agree("dep/package.json names reactive.ts");

    // A package.json edit: the directory's entry moves.
    warm.write("dep/package.json", "{ \"types\": \"./plain.ts\" }\n");
    let plain = warm.agree("dep/package.json names plain.ts");
    assert_ne!(plain, reads, "the package.json edit moved the resolution");
    warm.write("dep/package.json", "{ \"types\": \"./reactive.ts\" }\n");
    assert_eq!(
        warm.agree("dep/package.json names reactive.ts again"),
        reads
    );

    // A new file shadows the directory (`./dep.ts` before `./dep/`), and its
    // removal gives the resolution back.
    warm.write("dep.ts", PLAIN);
    let shadowed = warm.agree("dep.ts shadows dep/");
    assert_ne!(shadowed, reads, "the new file took the resolution");
    warm.remove("dep.ts");
    assert_eq!(warm.agree("dep.ts removed"), reads);

    // A `paths` change in the configuration.
    warm.write("App.tsx", &app("@dep", ""));
    let mapped = warm.agree("@dep maps to paths-a.ts");
    warm.write("tsconfig.json", &tsconfig("./paths-b.ts"));
    let remapped = warm.agree("@dep maps to paths-b.ts");
    assert_ne!(remapped, mapped, "the paths change moved the resolution");
    warm.write("tsconfig.json", &tsconfig("./paths-a.ts"));
    assert_eq!(warm.agree("@dep maps to paths-a.ts again"), mapped);

    // A symlink swapped to another target: the linked module's real path
    // and text move, the specifier does not.
    #[cfg(unix)]
    {
        symlink("variants/reactive", &warm.path("linked"));
        warm.write("App.tsx", &app("./linked", ""));
        let linked = warm.agree("linked -> variants/reactive");
        symlink("variants/plain", &warm.path("linked"));
        let swapped = warm.agree("linked -> variants/plain");
        assert_ne!(swapped, linked, "the symlink swap moved the resolution");
        symlink("variants/reactive", &warm.path("linked"));
        assert_eq!(warm.agree("linked -> variants/reactive again"), linked);
    }

    // Import edits with nothing else moved: each builds a fresh program chain
    // from the files the previous one parsed; then a parsed file's text
    // changes under an import edit, and so does a file's text with its
    // length and imports kept.
    warm.write("App.tsx", &app("./dep", ""));
    // The link's files are the include's too: gone, the program is the
    // first one again.
    #[cfg(unix)]
    fs::remove_file(warm.path("linked")).unwrap();
    assert_eq!(warm.agree("back to ./dep"), reads);
    warm.write(
        "App.tsx",
        &app(
            "./dep",
            "import { setCount } from \"./source\";\nexport const reset = () => setCount(0);",
        ),
    );
    let more = warm.agree("an import added");
    warm.write("App.tsx", &app("./dep", ""));
    assert_eq!(warm.agree("the import removed"), reads);
    warm.write("dep/reactive.ts", PLAIN);
    warm.write(
        "App.tsx",
        &app(
            "./dep",
            "import { count } from \"./source\";\nexport const peek = count;",
        ),
    );
    let edited = warm.agree("dep/reactive.ts is plain, under an import edit");
    assert_ne!(edited, more, "the edited dependency was parsed again");
    warm.write("dep/reactive.ts", &reactive(1));
    warm.write("App.tsx", &app("./dep", ""));
    assert_eq!(warm.agree("dep/reactive.ts restored"), reads);
}

/// ADR 0546: after an import edit the fresh chain rescans the exports of
/// only the modules that reach a file that changed; every other module keeps
/// the exported identities its file's parse gave. Re-export chains (`export
/// *`, a renamed `export { … } from`), a re-export whose target moves in the
/// same check as an import edit, a module that joins the global scope and a
/// deep dependency edited under an import edit: every warm answer is the
/// one-shot answer at the same path, and each move changes the answer.
#[test]
fn a_fresh_chain_keeps_only_exports_nothing_changed_reaches() {
    let Some(warm) = Warm::new("warm-export-rescan") else {
        return;
    };
    warm.write("tsconfig.json", &tsconfig("./paths-a.ts"));
    warm.write("dep/package.json", "{ \"types\": \"./reactive.ts\" }\n");
    warm.write("dep/reactive.ts", &reactive(1));
    warm.write("dep/plain.ts", PLAIN);
    warm.write("paths-a.ts", &reactive(0));
    warm.write("paths-b.ts", PLAIN);
    warm.write("reexport.ts", "export * from \"./dep\";\n");
    warm.write(
        "alias.ts",
        "export { readCount as readAgain, readCount } from \"./reexport\";\n",
    );
    warm.write("App.tsx", &app("./alias", ""));
    let reads = warm.agree("readCount through two re-exports");

    // An import edit: the chain is rebuilt, the re-exports keep their scans.
    let added = "import { readAgain } from \"./alias\";\nexport const again = readAgain;";
    warm.write("App.tsx", &app("./alias", added));
    let again = warm.agree("an import of the renamed re-export added");
    warm.write("App.tsx", &app("./alias", ""));
    assert_eq!(warm.agree("the import removed"), reads);

    // A re-export's target moves in the same check as an import edit.
    warm.write("reexport.ts", "export * from \"./paths-b\";\n");
    warm.write("App.tsx", &app("./alias", added));
    let moved = warm.agree("the star re-export now names paths-b.ts");
    assert_ne!(moved, again, "the re-export's move reached the importer");
    warm.write("reexport.ts", "export * from \"./dep\";\n");
    warm.write("App.tsx", &app("./alias", ""));
    assert_eq!(warm.agree("the star re-export names ./dep again"), reads);

    // A module that joins the global scope under an import edit.
    warm.write(
        "global.ts",
        "export {};\ndeclare global {\n  interface Window { counted: number }\n}\n",
    );
    warm.write("App.tsx", &app("./alias", added));
    warm.agree("a global augmentation joins under an import edit");
    warm.remove("global.ts");
    warm.write("App.tsx", &app("./alias", ""));
    assert_eq!(warm.agree("the augmentation removed"), reads);

    // A deep dependency's text changes under an import edit.
    warm.write("dep/reactive.ts", PLAIN);
    warm.write("App.tsx", &app("./alias", added));
    let plain = warm.agree("dep/reactive.ts is plain, under an import edit");
    assert_ne!(
        plain, again,
        "the edited dependency reached its re-exporters"
    );
    warm.write("dep/reactive.ts", &reactive(1));
    warm.write("App.tsx", &app("./alias", ""));
    assert_eq!(warm.agree("dep/reactive.ts restored"), reads);
}
