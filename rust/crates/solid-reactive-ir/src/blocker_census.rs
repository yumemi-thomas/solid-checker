//! ADR 1061: the blocker census. Each uncertifiable row names the first blocker its analysis met (the first writer of a global, the first problem of a
//! package's closure, one veto formula, the first row filed at a site). With `SOLID_CHECKER_BLOCKER_CENSUS=1` the analyses that stop there keep
//! collecting, into a side table of the internal JSON (`blockerCensus`) that names every blocker they found. Internal only (ADR 0328): no finding, verdict,
//! text output or exit code moves, and nothing reads the table back. A row of it is joined to a finding by location by whoever reads the JSON.

use serde::{Deserialize, Serialize};
use solid_facts::package_reach::{PackageReach, PackageReachIndex};

pub use solid_facts::package_reach::blocker_census as enabled;

/// Every blocker one analysis found at one place.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CensusRow {
    pub path: String,
    pub start: u64,
    pub end: u64,
    /// The blocker kind of the row the analysis filed (`package-may-call-action`), or the kind a dropped row would have named.
    pub kind: String,
    /// What it found, each `family|…` (`writer|any|<root>`, `problem|<root>|<word>|<detail>`, `formula|<name>|<hazard>`, `dropped|<kind>`).
    pub blockers: Vec<String>,
}

impl CensusRow {
    pub(crate) fn at(location: &crate::Location, kind: &str, blockers: Vec<String>) -> Self {
        Self {
            path: location.path.as_ref().to_owned(),
            start: location.start_byte,
            end: location.end_byte,
            kind: kind.to_owned(),
            blockers,
        }
    }
}

/// The census of one package root: every problem of its closure, and, for a writer under an unfixed key, the files that write so.
pub(crate) fn root_blockers(index: &PackageReachIndex, root: &str, out: &mut Vec<String>) {
    match index.census.get(root) {
        Some(census) => {
            for problem in &census.problems {
                out.push(problem_text(root, problem));
            }
        }
        None => {
            if let Some(reach) = index.by_root.get(root) {
                out.push(problem_text(root, reach));
            }
        }
    }
}

/// The files of `root`'s closure that write a global under a key nothing decides.
pub(crate) fn write_any_blockers(index: &PackageReachIndex, root: &str, out: &mut Vec<String>) {
    if let Some(census) = index.census.get(root) {
        for file in &census.write_any_files {
            out.push(format!("write-any|{root}|{file}"));
        }
        if census.write_any_unread {
            out.push(format!("write-any|{root}|unread"));
        }
    }
}

pub(crate) fn problem_text(root: &str, problem: &PackageReach) -> String {
    match problem {
        PackageReach::Clear => format!("problem|{root}|clear|"),
        PackageReach::ReachesSolid { via } => format!("problem|{root}|reaches-solid|{via}"),
        PackageReach::Undecided { blocker, detail } => {
            format!("problem|{root}|{}|{detail}", blocker.word())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::package_reach::{ReachBlocker, RootCensus};

    #[test]
    fn a_root_names_every_problem_of_its_closure_and_its_any_key_files() {
        let mut index = PackageReachIndex::default();
        index.by_root.insert(
            "/n/node_modules/p".into(),
            PackageReach::ReachesSolid {
                via: "p -> solid-js".into(),
            },
        );
        index.census.insert(
            "/n/node_modules/p".into(),
            RootCensus {
                problems: vec![
                    PackageReach::ReachesSolid {
                        via: "p -> solid-js".into(),
                    },
                    PackageReach::Undecided {
                        blocker: ReachBlocker::CodeGeneration,
                        detail: "p/a.js: eval".into(),
                    },
                ],
                write_any_files: vec!["p/b.js".into()],
                write_any_unread: true,
            },
        );
        let mut out = Vec::new();
        write_any_blockers(&index, "/n/node_modules/p", &mut out);
        root_blockers(&index, "/n/node_modules/p", &mut out);
        assert_eq!(
            out,
            [
                "write-any|/n/node_modules/p|p/b.js",
                "write-any|/n/node_modules/p|unread",
                "problem|/n/node_modules/p|reaches-solid|p -> solid-js",
                "problem|/n/node_modules/p|code-generation|p/a.js: eval",
            ]
        );
        // Without a census entry the verdict alone stands.
        index.census.clear();
        let mut out = Vec::new();
        root_blockers(&index, "/n/node_modules/p", &mut out);
        assert_eq!(
            out,
            ["problem|/n/node_modules/p|reaches-solid|p -> solid-js"]
        );
    }
}
