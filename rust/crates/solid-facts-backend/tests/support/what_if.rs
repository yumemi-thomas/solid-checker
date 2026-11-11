//! ADR 0950: what the default-build test and the what-if build's tests both look at: the fixture they run, the strings only a what-if binary holds, and the traces a
//! daemon leaves (so the test that the what-if binary leaves none is shown to see them where a default build makes one).
use std::{
    fs,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::Command,
};

/// The program both builds check: `patch(Number, "isFinite", f)` through a helper writing its parameter under an unspelled key, so the row's veto
/// (`handed-root-patch`) stands on `OpenMemberWrite(*)`, `StandardImpostor(*)`, `StandardRootHanded` and `StandardRootReceived`.
pub const FIXTURE: &str = "handed-root-patch";

pub fn fixture(name: &str) -> PathBuf {
    // Canonical, so a daemon's command line (which names the canonical project) is the one `daemon_pids` searches for.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
        .canonicalize()
        .unwrap()
}

/// Strings in the what-if build's code and nowhere in a default build's: the unknown-hazard error, the refusal of the formats, the missing-value error and the refusal of the analysis logs.
pub const WHAT_IF_MARKERS: [&str; 4] = [
    "names no hazard fact",
    "answers the internal JSON only",
    "--assume needs hazard ids",
    "writes no analysis log",
];

pub fn contains(haystack: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// A temporary directory with a path short enough for a daemon's socket in it (a socket path must fit `SUN_LEN`, which a long `TMPDIR` does not: the daemon
/// would exit before it listened, and a probe for it would see nothing, whatever the build; `daemon_residue_process.rs` keeps its sockets under `/tmp` too).
pub fn socket_directory(label: &str) -> PathBuf {
    let path = PathBuf::from(format!("/tmp/wi-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

/// What a retained daemon leaves for the check of `tsconfig` run with `temp` as its temporary directory: a socket anywhere under it, a process serving
/// the project. Empty when the check was one-shot.
pub fn daemon_traces(temp: &Path, tsconfig: &Path) -> Vec<String> {
    fn sockets(directory: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(directory).into_iter().flatten().flatten() {
            let kind = entry.file_type().unwrap();
            if kind.is_socket() {
                found.push(format!("socket {}", entry.path().display()));
            } else if kind.is_dir() {
                sockets(&entry.path(), found);
            }
        }
    }
    let mut traces = Vec::new();
    sockets(temp, &mut traces);
    traces.extend(
        daemon_pids(tsconfig)
            .into_iter()
            .map(|pid| format!("process {pid}")),
    );
    traces
}

/// The pids of the daemons serving `tsconfig`.
pub fn daemon_pids(tsconfig: &Path) -> Vec<String> {
    let output = Command::new("pgrep")
        .args(["-f", "--"])
        .arg(format!("--serve --project {}", tsconfig.display()))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}
