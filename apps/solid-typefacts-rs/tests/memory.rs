//! Memory behavior of the tsrs backend over a long session: 300 full rebuilds
//! and 100 incremental updates must not accumulate (plan section 3.2: without
//! the `Generation` machinery a small project leaks about 9 MB per rebuild and
//! 1.4 MB per incremental update).
//!
//! RSS is process-wide, so both loops run in one test, sequentially. Under
//! `TSRS_ARENA_POISON=1` freed memory is filled and never reused, so growth is
//! expected; the loops then only prove that nothing reads freed memory.

use solid_typefacts::backend::ClosureBackend;
use solid_typefacts::context::Context;
use solid_typefacts::project::Project;
use std::path::PathBuf;
use typefacts::v3::FileChange;

fn rss_kib() -> u64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                return rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
    }
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn change(path: &str, version: u64, source: &str) -> FileChange {
    FileChange {
        path: path.to_owned(),
        source: source.as_bytes().to_vec(),
        deleted: false,
        version,
    }
}

fn poisoned() -> bool {
    std::env::var_os("TSRS_ARENA_POISON").is_some_and(|value| value == "1")
}

#[test]
fn rebuild_and_update_loops_do_not_accumulate() {
    let root =
        std::env::temp_dir().join(format!("solid-typefacts-rs-memory-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let dir = Dir(root.canonicalize().unwrap());
    let write = |name: &str, text: &str| std::fs::write(dir.0.join(name), text).unwrap();
    write(
        "tsconfig.json",
        r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}"#,
    );
    write("a.ts", "export function a(): number { return 0; }\n");
    write(
        "b.ts",
        "import { a } from \"./a\";\nexport const b = a();\n",
    );
    write("c.ts", "export const c = [1, 2, 3].map((n) => n * 2);\n");
    let path = |name: &str| dir.0.join(name).to_string_lossy().into_owned();
    let (a, b) = (path("a.ts"), path("b.ts"));

    let ctx = Context::new();
    let mut project = Project::open(&ctx, &path("tsconfig.json"), None, None).unwrap();
    let start = rss_kib();

    // 300 full rebuilds: two files changed at once take the rebuild path.
    let (mut at_30, mut at_300) = (0, 0);
    for round in 1..=300_u64 {
        project
            .update(
                &ctx,
                &[
                    change(
                        &a,
                        round,
                        &format!("export function a(): number {{ return {round}; }}\n"),
                    ),
                    change(
                        &b,
                        round,
                        &format!("import {{ a }} from \"./a\";\nexport const b = a() + {round};\n"),
                    ),
                ],
            )
            .unwrap();
        assert_eq!(
            project.last_update.kind,
            Some(solid_typefacts::unsafe_boundary::UpdateKind::Rebuilt)
        );
        project.release_analysis_state();
        if round == 30 {
            at_30 = rss_kib();
        }
        if round == 300 {
            at_300 = rss_kib();
        }
    }
    eprintln!("rebuilds: start {start} KiB, #30 {at_30} KiB, #300 {at_300} KiB");

    // 100 incremental updates (one file each; the rebase policy runs once).
    let before_updates = rss_kib();
    let (mut at_10, mut at_100) = (0, 0);
    for round in 1..=100_u64 {
        project
            .update(
                &ctx,
                &[change(
                    &a,
                    1000 + round,
                    &format!("export function a(): number {{ return {round}; }}\nexport const edit{round} = {round};\n"),
                )],
            )
            .unwrap();
        project.release_analysis_state();
        if round == 10 {
            at_10 = rss_kib();
        }
        if round == 100 {
            at_100 = rss_kib();
        }
    }
    eprintln!("updates: before {before_updates} KiB, #10 {at_10} KiB, #100 {at_100} KiB");

    if poisoned() {
        eprintln!("TSRS_ARENA_POISON=1: growth is expected, no RSS bound asserted");
        return;
    }
    // Without freeing, 270 more rebuilds would add ~2.4 GiB and 90 more updates
    // ~126 MiB. The bounds leave room for allocator noise and the decaying
    // warm-up the spike measured (about 0.05 MiB per rebuild by #250).
    assert!(
        at_300.saturating_sub(at_30) < 96 * 1024,
        "270 rebuilds grew RSS by {} KiB",
        at_300.saturating_sub(at_30)
    );
    assert!(
        at_300 < 512 * 1024,
        "absolute RSS after 300 rebuilds: {at_300} KiB"
    );
    assert!(
        at_100.saturating_sub(at_10) < 48 * 1024,
        "90 updates grew RSS by {} KiB",
        at_100.saturating_sub(at_10)
    );
}
