//! ADR 0975: reviewed package behavior, pinned to the bytes that were read.
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
/// `vitest-4.1` (ADR 0975): `expect(value).matcher(arguments)` for the matchers and values `solid_reactive_ir::reviewed_api` names, and `vi.fn` and `vi.stubGlobal`
/// registrations. Read on vitest 4.1.11, whose files are, byte for byte, 4.1.6's and 4.1.7's but for the runner's chunk (the reading rests on its `getCurrentTest`, the same
/// three lines in each of the three).
///
/// ADR 1000: the file an import of the package loads is part of the reading. Each package's manifest is pinned (one digest per release: they differ in the version and
/// dependency fields only; `type`, `main`, `module`, `exports`, `imports` and `browser` are the same in each), and so are both entries of `vitest` itself
/// (`dist/index.js`, which names the chunk's `expect` and `vi`, and `index.cjs`, which only throws). A patched entry or `exports` map is no match.
pub const PREMISES: &[ReviewedPremise] = &[
    STYLEX_0_19,
    ReviewedPremise {
        id: "vitest-4.1",
        entry: "vitest",
        files: &[
            ReviewedFile {
                package: "vitest",
                path: "package.json",
                sha256: &[
                    "16e78981f0c4f0b28c7056c06c0d87ee7c65821c721915df941996437a4267a9",
                    "7cd821c11d3dcf6b9f2a53bf06d8ba645d7dd0f6be846764917ee694cf3eafee",
                    "a28126d97bcaf567da5bed69443b7f3bcd9a7a8c38c8b66e554686b6bb2c10e0",
                ],
            },
            ReviewedFile {
                package: "vitest",
                path: "dist/index.js",
                sha256: &["3986618dbcbbdc0a3e200e2a507a434ef6f13dd8cc20c234168edb7b16e98d29"],
            },
            ReviewedFile {
                package: "vitest",
                path: "index.cjs",
                sha256: &["15a78ebcf7fc24ea0a829448d770ca486b7506fcf8fd696a2cbf8e17d25dc02b"],
            },
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
                path: "package.json",
                sha256: &[
                    "40ce909a274d4945f5fe440d9a05fab5b3cef5d586cba04e8b00f6386b06e083",
                    "583dee9a3567dbb562033ac3e08dbe64a97791f78e37a68f45137fe7a0594dfd",
                    "bc566b4322c25920e82c2fcdea00c2154d8ef98dd30de6605c5f89ac0d607adc",
                ],
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
                path: "package.json",
                sha256: &[
                    "8d00db5190169f01f7d83758802bbf0db2796a7e3b0700bb9808d2312b5820fa",
                    "c45024b2d922d19dfc72f7f064c4184ad2e291bc37293e15a2cdc0fa88675ff5",
                    "9ae07df2e84098cc2360a9e1563925e123c8740e4b7d158bab11fa20573d6516",
                ],
            },
            ReviewedFile {
                package: "@vitest/spy",
                path: "dist/index.js",
                sha256: &["125bd25008172821d8d88c96fa2ebd189d52bf1494302486669dde9a4bc5c62d"],
            },
            ReviewedFile {
                package: "chai",
                path: "package.json",
                sha256: &["b1f1454787c0ebc1a4324822bd6080e12535b1ab379b2c49d4eb467623123476"],
            },
            ReviewedFile {
                package: "chai",
                path: "index.js",
                sha256: &["5261e5ed7e75365c2a5d832c220ff409daf704a3088b5511de34694e881d5edb"],
            },
            ReviewedFile {
                package: "@vitest/runner",
                path: "package.json",
                sha256: &[
                    "349bce64ba4aaf8e0b0afcc21af521ed1ddec22ec06657c0eb838d1503d76ad8",
                    "992f85c03e56ea4ce920b42a0ffe1bc140c0acd9df99562deb05f348d8408e23",
                    "357e56773a82d3aa95bf920a54ea7f91575d02e6b156d9d7719ea485708e6837",
                ],
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
                path: "package.json",
                sha256: &[
                    "c1e7a62430b76f5e65197ba2dbd2be65fed1f308f9b77288237167f1638f565f",
                    "ee1321f307c5a293cea312b682e4c7b6afba2dc68e620e54ee30215e09165939",
                    "60611b68702749e1da829d835b528bf5dfe6712e4d59be14d058fcb46cf7a6e8",
                ],
            },
            ReviewedFile {
                package: "@vitest/utils",
                path: "dist/helpers.js",
                sha256: &["1a5e6ae2acfeb0477419d8e5bee9f89b873b7ef8b20d13490ea95e20615cf1ec"],
            },
        ],
    },
];

/// `stylex-0.19` (ADR 1063): in `@stylexjs/stylex` 0.19.0's runtime, both entries (`lib/es/stylex.mjs`, the `import` condition, and `lib/cjs/stylex.js`, `require`)
/// define `create`, `createTheme`, `defineConsts`, `defineVars`, `defineMarker`, `defaultMarker`, `firstThatWorks`, `keyframes`, `positionTry`,
/// `viewTransitionClass`, `unstable_conditional`, `unstable_defineVarsNested`, `unstable_defineConstsNested` and `unstable_createThemeNested` as functions whose
/// whole body is `throw errorForFn(name)` (`new Error(…)`: the message names the Babel plugin that compiles the calls away), and export them under those names.
/// A call of one returns no value. The declaration file is pinned too: the analysis names these by the declarations the compiler resolves in it.
const STYLEX_0_19: ReviewedPremise = ReviewedPremise {
    id: "stylex-0.19",
    entry: "@stylexjs/stylex",
    files: &[
        ReviewedFile {
            package: "@stylexjs/stylex",
            path: "package.json",
            sha256: &["175af0021645f444babee4f1174c8d6beb2ea03a6e9b73875c24681782912dfb"],
        },
        ReviewedFile {
            package: "@stylexjs/stylex",
            path: "lib/es/stylex.mjs",
            sha256: &["6b68bd82495f031829b35106798dd81bb307c5bf97180ee13b4ef56dddf05740"],
        },
        ReviewedFile {
            package: "@stylexjs/stylex",
            path: "lib/cjs/stylex.js",
            sha256: &["334ea707296b1c526dd611b4d5398f009649257f5d24978b27d93d050a479fd8"],
        },
        ReviewedFile {
            package: "@stylexjs/stylex",
            path: "lib/es/stylex.d.ts",
            sha256: &["ea52ff717cdb84e014a3f26f94bf322c191b5f49207adf3724fda0ac527e8d22"],
        },
    ],
};

/// The directory of the package `name` that Node finds from the directory `from`, canonical.
fn package_dir(from: &Path, name: &str) -> Option<PathBuf> {
    find_package_root(&from.join("package.json"), name, None)?
        .canonicalize()
        .ok()
}

/// What says a file's bytes are the bytes an answer was read from: its length, its modification time, its inode and the inode's change time.
pub(crate) type FileStamp = (u64, Option<SystemTime>, u64, i64, i64);

/// The stamp of a file, or `None` where the platform gives no change time, and the answer is then read again each time. Length and modification time are not enough
/// (ADR 1000): a package manager extracts every file with one fixed modification time, and a patch that keeps the length keeps both. No program sets the change time.
pub(crate) fn file_stamp(metadata: &fs::Metadata) -> Option<FileStamp> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((
            metadata.len(),
            metadata.modified().ok(),
            metadata.ino(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

/// The lower-case hex SHA-256 of a file, kept for the process while its stamp holds.
fn digest_of(path: &Path) -> Option<String> {
    type Kept = Mutex<HashMap<PathBuf, (FileStamp, String)>>;
    static KEPT: OnceLock<Kept> = OnceLock::new();
    let stamp = file_stamp(&fs::metadata(path).ok()?);
    let kept = KEPT.get_or_init(Mutex::default);
    if let Some(stamp) = stamp
        && let Some((held, digest)) = kept.lock().ok()?.get(path)
        && *held == stamp
    {
        return Some(digest.clone());
    }
    let bytes = fs::read(path).ok()?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if let Some(stamp) = stamp {
        kept.lock()
            .ok()?
            .insert(path.to_path_buf(), (stamp, digest.clone()));
    }
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

    /// ADR 1000: a rewrite that keeps the file's length and puts its modification time back (what a patch of an installed package does: every file is extracted with
    /// one fixed time) is read again, not answered from the digest kept for the old bytes.
    #[test]
    fn a_rewrite_of_the_same_length_and_modification_time_is_hashed_again() {
        let directory = project();
        let file = directory.join("pinned.js");
        fs::write(&file, TWO).unwrap();
        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        assert_eq!(digest_of(&file), Some(two_digest()));
        // The change time has the clock's granularity.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let other = TWO.replace("42", "43");
        fs::write(&file, &other).unwrap();
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let metadata = fs::metadata(&file).unwrap();
        assert_eq!(metadata.len(), TWO.len() as u64);
        assert_eq!(metadata.modified().unwrap(), modified);
        assert_eq!(
            digest_of(&file),
            Some(format!("{:x}", Sha256::digest(other.as_bytes())))
        );
        let _ = fs::remove_dir_all(&directory);
    }

    /// ADR 1000: the file an import loads is part of the reading, so each package of a premise pins its manifest.
    #[test]
    fn every_package_of_a_premise_pins_its_manifest() {
        for premise in PREMISES {
            let packages = premise
                .files
                .iter()
                .map(|file| file.package)
                .collect::<BTreeSet<_>>();
            for package in packages {
                assert!(
                    premise
                        .files
                        .iter()
                        .any(|file| file.package == package && file.path == "package.json"),
                    "{}: {package} has no pinned manifest",
                    premise.id
                );
            }
        }
    }

    /// The published files of `fixtures/reactive-ir/reviewed-vitest-4-1` that the premise pins, copied to a new directory; the root of `vitest` there.
    fn published_vitest(directory: &Path) -> BTreeMap<String, PathBuf> {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/reviewed-vitest-4-1/node_modules");
        let vitest = PREMISES
            .iter()
            .find(|premise| premise.id == "vitest-4.1")
            .expect("the vitest premise");
        for file in vitest.files {
            let below = Path::new("node_modules").join(file.package).join(file.path);
            let bytes = fs::read(fixture.join(file.package).join(file.path))
                .unwrap_or_else(|error| panic!("{} {}: {error}", file.package, file.path));
            fs::create_dir_all(directory.join(&below).parent().unwrap()).unwrap();
            fs::write(directory.join(&below), bytes).unwrap();
        }
        let root = directory.join("node_modules/vitest");
        BTreeMap::from([(root.to_string_lossy().into_owned(), root)])
    }

    /// ADR 1000: the published bytes hold the premise, and a patch of the file an import loads (the entry, or the manifest's `exports` of the entry package or of a
    /// package the reading rests on) holds nothing, though every file ADR 0975 pinned is as it was.
    #[test]
    fn a_patched_entry_or_manifest_of_vitest_holds_nothing() {
        let directory = project();
        let roots = published_vitest(&directory);
        let found = verified_premises(&roots, PREMISES);
        assert!(
            found.values().any(|ids| ids.contains("vitest-4.1")),
            "{found:?}"
        );
        let _ = fs::remove_dir_all(&directory);
        for patched in [
            "vitest/dist/index.js",
            "vitest/index.cjs",
            "vitest/package.json",
            "@vitest/expect/package.json",
            "@vitest/spy/package.json",
            "chai/package.json",
        ] {
            let directory = project();
            let roots = published_vitest(&directory);
            let file = directory.join("node_modules").join(patched);
            let mut bytes = fs::read(&file).unwrap();
            bytes.push(b'\n');
            fs::write(&file, bytes).unwrap();
            assert!(
                verified_premises(&roots, PREMISES).is_empty(),
                "{patched} patched still holds the premise"
            );
            let _ = fs::remove_dir_all(&directory);
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
