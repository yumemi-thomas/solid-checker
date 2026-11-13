//! ADR 0974: reviewed package behavior, pinned to the bytes that were read.
//!
//! A package the build does not read as source may call a Solid action, so each call into it is a site (ADR 0972). Where the source of a package was *read*
//! for one API (what `expect(value).toBe(1)` calls, on a value no foreign accessor can run in), that reading is a premise: recorded in the ADR, named by an id
//! here, and applied by the analysis ([`solid_reactive_ir`]'s `reviewed_api`) only where the installed bytes are the bytes that were read. The pin is a digest
//! of each file the reading rests on, in the package the premise names it in, found by Node's lookup from the entry package's directory. A version nobody
//! read, a patched file, a file moved: no match, no premise, and the sites stand.
//!
//! The analysis never reads the filesystem; the verdict travels in the package boundary ([`solid_facts::package_reach::PackageReachIndex::reviewed`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::host_constants::find_package_root;

/// One file a premise rests on.
pub struct ReviewedFile {
    /// The manifest name of the package the file is in (`@vitest/expect`).
    pub package: &'static str,
    /// The file's path below that package's root.
    pub path: &'static str,
    /// The lower-case hex SHA-256 of its bytes: one per version of the file that was read (a file several releases of a package ship unchanged has one; one that
    /// changed around the part the reading rests on has each version's, after the reading checked that part in each).
    pub sha256: &'static [&'static str],
}

/// A reading of package source: the id the analysis names it by, the package it starts from and every file it rests on.
pub struct ReviewedPremise {
    pub id: &'static str,
    /// The manifest name of the package whose directory the other files are looked up from.
    pub entry: &'static str,
    pub files: &'static [ReviewedFile],
}

/// Every premise. A premise is added only with its reading written in `docs/decisions.md`.
///
/// `vitest-4.1` (ADR 0974): `expect(value).matcher(arguments)` for the matchers and values `solid_reactive_ir::reviewed_api` names, and `vi.fn` and `vi.stubGlobal`
/// registrations. Read on vitest 4.1.11, whose files are, byte for byte, 4.1.6's and 4.1.7's but for the runner's chunk (the reading rests on its `getCurrentTest`, the same
/// three lines in each of the three).
pub const PREMISES: &[ReviewedPremise] = &[ReviewedPremise {
    id: "vitest-4.1",
    entry: "vitest",
    files: &[
        ReviewedFile {
            package: "vitest",
            path: "dist/index.d.ts",
            sha256: &["90ba95a763101bb61b8a799731a2ed60b5016b8135c1a2d5186862d4b534d4a1"],
        },
        ReviewedFile {
            package: "vitest",
            path: "dist/chunks/test.DNmyFkvJ.js",
            sha256: &["f07fc8036b066834143b14748bd70c436a6a2f15f648455af8d3a003a4eafc94"],
        },
        ReviewedFile {
            package: "vitest",
            path: "dist/chunks/utils.BX5Fg8C4.js",
            sha256: &["f7b093e5d0814fbf4a58675cd3448a7dea4359926cbc0d02f7621de3b5a1f498"],
        },
        ReviewedFile {
            package: "@vitest/expect",
            path: "dist/index.d.ts",
            sha256: &["5c935b7fc4ddc1410ea1cd7cd4e35ed106a6e4920dd27a9480a40fd224359dc3"],
        },
        ReviewedFile {
            package: "@vitest/expect",
            path: "dist/index.js",
            sha256: &["b177ca97952f1d65d3d2691edbc31ab1af56ac36b3cf08d7f56ad0ca69f07a45"],
        },
        ReviewedFile {
            package: "@vitest/spy",
            path: "dist/index.js",
            sha256: &["125bd25008172821d8d88c96fa2ebd189d52bf1494302486669dde9a4bc5c62d"],
        },
        ReviewedFile {
            package: "chai",
            path: "index.js",
            sha256: &["5261e5ed7e75365c2a5d832c220ff409daf704a3088b5511de34694e881d5edb"],
        },
        ReviewedFile {
            package: "@vitest/runner",
            path: "dist/chunk-artifact.js",
            sha256: &[
                "bf6af6d2f4469911879a67e83b8f9e7b20332cf42795032c53457b10ccc1c617",
                "c10a7a80fca65fb42e27eabf7655c5ce5ecf36e8aaeb1811499efd063df62556",
                "25a9a7575905cbd4811421f3211e498e55b6e7c303d0ed9722f21588b3a6fc2d",
            ],
        },
        ReviewedFile {
            package: "@vitest/runner",
            path: "dist/index.js",
            sha256: &["61a758d5e6d41e0928c92bfd5ab7152bde625d90b97a08b269dc5a0438485d35"],
        },
        ReviewedFile {
            package: "@vitest/utils",
            path: "dist/helpers.js",
            sha256: &["1a5e6ae2acfeb0477419d8e5bee9f89b873b7ef8b20d13490ea95e20615cf1ec"],
        },
    ],
}];

/// The directory of the package `name` that Node finds from the directory `from`, canonical.
fn package_dir(from: &Path, name: &str) -> Option<PathBuf> {
    find_package_root(&from.join("package.json"), name, None)?
        .canonicalize()
        .ok()
}

/// The lower-case hex SHA-256 of a file, kept for the process while its length and modification time hold.
fn digest_of(path: &Path) -> Option<String> {
    type Kept = Mutex<HashMap<PathBuf, ((u64, Option<SystemTime>), String)>>;
    static KEPT: OnceLock<Kept> = OnceLock::new();
    let metadata = fs::metadata(path).ok()?;
    let stamp = (metadata.len(), metadata.modified().ok());
    let kept = KEPT.get_or_init(Mutex::default);
    if let Some((held, digest)) = kept.lock().ok()?.get(path)
        && *held == stamp
    {
        return Some(digest.clone());
    }
    let bytes = fs::read(path).ok()?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    kept.lock()
        .ok()?
        .insert(path.to_path_buf(), (stamp, digest.clone()));
    Some(digest)
}

/// The directories of the packages of `premise` when every file matches its digest, keyed by package name; `None` when a package is not found, a file is
/// missing or one differs.
fn verified_packages(
    entry_dir: &Path,
    premise: &ReviewedPremise,
) -> Option<BTreeMap<&'static str, PathBuf>> {
    let mut directories = BTreeMap::<&'static str, PathBuf>::new();
    directories.insert(premise.entry, entry_dir.canonicalize().ok()?);
    for file in premise.files {
        if !directories.contains_key(file.package) {
            directories.insert(file.package, package_dir(entry_dir, file.package)?);
        }
        let digest = digest_of(&directories[file.package].join(file.path))?;
        if !file.sha256.contains(&digest.as_str()) {
            return None;
        }
    }
    Some(directories)
}

/// The premises that hold under the package directories `roots` (installed roots by the spelling the compiler's paths use): for each root named by a verified
/// premise, the ids that hold there. A premise starts from the root whose manifest names its entry package; the packages it rests on are the roots it is
/// recorded under, when the program names them.
#[must_use]
pub fn verified_premises(
    roots: &BTreeMap<String, PathBuf>,
    premises: &[ReviewedPremise],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut found = BTreeMap::<String, BTreeSet<String>>::new();
    for premise in premises {
        for (spelled, directory) in roots {
            let Some(manifest) = fs::read(directory.join("package.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            else {
                continue;
            };
            if manifest.get("name").and_then(serde_json::Value::as_str) != Some(premise.entry) {
                continue;
            }
            let Some(packages) = verified_packages(directory, premise) else {
                continue;
            };
            found
                .entry(spelled.clone())
                .or_default()
                .insert(premise.id.to_owned());
            // The other packages of the premise, where the program names them.
            for (other, other_directory) in roots {
                if packages
                    .values()
                    .any(|verified| verified == other_directory)
                {
                    found
                        .entry(other.clone())
                        .or_default()
                        .insert(premise.id.to_owned());
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn project() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-reviewed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory.canonicalize().unwrap()
    }

    const TWO: &str = "export const answer = 42;";
    // sha256("export const answer = 42;")
    fn two_digest() -> String {
        format!("{:x}", Sha256::digest(TWO.as_bytes()))
    }

    fn premise(sha: &'static str) -> ReviewedPremise {
        let alternatives: &'static [&'static str] = Box::leak(Box::new([sha, "0000"]));
        let files: &'static [ReviewedFile] = Box::leak(Box::new([
            ReviewedFile {
                package: "rev-main",
                path: "dist/index.js",
                sha256: alternatives,
            },
            ReviewedFile {
                package: "rev-dep",
                path: "lib.js",
                sha256: alternatives,
            },
        ]));
        ReviewedPremise {
            id: "rev-test",
            entry: "rev-main",
            files,
        }
    }

    fn installed(directory: &Path, second: &str) -> BTreeMap<String, PathBuf> {
        write(
            directory,
            "node_modules/rev-main/package.json",
            r#"{"name":"rev-main"}"#,
        );
        write(directory, "node_modules/rev-main/dist/index.js", TWO);
        write(
            directory,
            "node_modules/rev-dep/package.json",
            r#"{"name":"rev-dep"}"#,
        );
        write(directory, "node_modules/rev-dep/lib.js", second);
        let main = directory.join("node_modules/rev-main");
        let dep = directory.join("node_modules/rev-dep");
        BTreeMap::from([
            (main.to_string_lossy().into_owned(), main),
            (dep.to_string_lossy().into_owned(), dep),
        ])
    }

    /// A digest that is not 64 lower-case hex characters can never match, and a premise with one holds nowhere without saying so: the table is checked as data.
    #[test]
    fn the_table_is_well_formed() {
        let mut ids = BTreeSet::new();
        for premise in PREMISES {
            assert!(ids.insert(premise.id), "{} is listed twice", premise.id);
            assert!(
                premise
                    .files
                    .iter()
                    .any(|file| file.package == premise.entry),
                "{} names no file of its entry package",
                premise.id
            );
            for file in premise.files {
                assert!(!file.sha256.is_empty(), "{} {}", file.package, file.path);
                for digest in file.sha256 {
                    assert!(
                        digest.len() == 64
                            && digest
                                .chars()
                                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                        "{} {}: {digest}",
                        file.package,
                        file.path
                    );
                }
            }
        }
    }

    #[test]
    fn a_premise_holds_where_every_file_matches_its_digest() {
        let directory = project();
        let roots = installed(&directory, TWO);
        let digest: &'static str = Box::leak(two_digest().into_boxed_str());
        let found = verified_premises(&roots, &[premise(digest)]);
        // The entry package and the package it rests on both carry the id.
        assert_eq!(found.len(), 2, "{found:?}");
        for ids in found.values() {
            assert!(ids.contains("rev-test"));
        }
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_file_that_differs_a_package_that_is_missing_and_an_unnamed_entry_hold_nothing() {
        let digest: &'static str = Box::leak(two_digest().into_boxed_str());
        // One byte differs.
        let directory = project();
        let roots = installed(&directory, "export const answer = 43;");
        assert!(verified_premises(&roots, &[premise(digest)]).is_empty());
        let _ = fs::remove_dir_all(&directory);
        // The package a file is in is not installed.
        let directory = project();
        let roots = installed(&directory, TWO);
        fs::remove_dir_all(directory.join("node_modules/rev-dep")).unwrap();
        assert!(verified_premises(&roots, &[premise(digest)]).is_empty());
        let _ = fs::remove_dir_all(&directory);
        // No root is named for the premise's entry.
        let directory = project();
        let mut roots = installed(&directory, TWO);
        roots.retain(|spelled, _| spelled.ends_with("rev-dep"));
        assert!(verified_premises(&roots, &[premise(digest)]).is_empty());
        let _ = fs::remove_dir_all(&directory);
    }
}
