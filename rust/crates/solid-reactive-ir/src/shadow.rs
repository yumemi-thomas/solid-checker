//! Shadow mode: proof that a retained (incremental) build equals a fresh one.
//!
//! After every incremental build, [`crate::IncrementalBuilder`] can rebuild the
//! same inputs with empty caches and compare the two [`Program`]s table by
//! table, row by row. Every later reuse step (per-file chunks, memoized
//! tables) is accepted only while this comparison stays silent.
//!
//! `SOLID_CHECKER_IR_SHADOW` selects the mode:
//! - unset, empty, `0` or `off`: no comparison (the default; the cost is one
//!   environment read at construction);
//! - `panic`: a mismatch panics naming the first differing table and row
//!   (process tests, `.cargo/config.toml`);
//! - any other value (`1`, `log`): a mismatch is logged, the retained caches
//!   are dropped, and the fresh program is served (the daemon, fail closed).
//!   `SOLID_CHECKER_IR_SHADOW_LOG=<path>` also appends the report to a file,
//!   because a daemon's stderr goes nowhere.

use std::{io::Write, time::Duration};

use crate::Program;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShadowMode {
    #[default]
    Off,
    /// Log the mismatch and serve the fresh program.
    Serve,
    /// Panic on the mismatch.
    Panic,
}

impl ShadowMode {
    #[must_use]
    pub fn from_setting(setting: Option<&str>) -> Self {
        match setting.map(str::trim) {
            None | Some("" | "0" | "off" | "false") => Self::Off,
            Some("panic") => Self::Panic,
            Some(_) => Self::Serve,
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        Self::from_setting(std::env::var("SOLID_CHECKER_IR_SHADOW").ok().as_deref())
    }
}

/// The first place two programs differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowMismatch {
    /// The `Program` table, named as its field.
    pub table: &'static str,
    /// The first differing row, or `None` for a table-level difference.
    pub row: Option<usize>,
    pub incremental_rows: usize,
    pub fresh_rows: usize,
    /// Canonical serializations of the differing rows; empty past a table's end.
    pub incremental: String,
    pub fresh: String,
}

impl std::fmt::Display for ShadowMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IR shadow mismatch: table `{}`", self.table,)?;
        if let Some(row) = self.row {
            write!(f, " row {row}")?;
        }
        write!(
            f,
            " (incremental {} rows, fresh {} rows)\n  incremental: {}\n  fresh:       {}",
            self.incremental_rows, self.fresh_rows, self.incremental, self.fresh
        )
    }
}

/// Canonical rows of a table: one string per row, in table order. Tables are
/// ordered `Vec`s by contract (final ordering sorts them); a `HashSet` is
/// sorted here.
fn row<T: serde::Serialize>(row: &T) -> String {
    serde_json::to_string(row).unwrap_or_else(|error| format!("<{error}>"))
}

/// [`compare`] over two tables' serialized rows, one row pair at a time: a
/// whole table's rows are never held as strings (the idle check compares
/// every table of two programs while both builds are resident).
fn compare_tables<T: serde::Serialize>(
    table: &'static str,
    incremental: &[T],
    fresh: &[T],
) -> Option<ShadowMismatch> {
    let at = incremental
        .iter()
        .zip(fresh)
        .position(|(left, right)| row(left) != row(right))
        .or_else(|| {
            (incremental.len() != fresh.len()).then(|| incremental.len().min(fresh.len()))
        })?;
    Some(ShadowMismatch {
        table,
        row: Some(at),
        incremental_rows: incremental.len(),
        fresh_rows: fresh.len(),
        incremental: incremental.get(at).map(row).unwrap_or_default(),
        fresh: fresh.get(at).map(row).unwrap_or_default(),
    })
}

fn compare(
    table: &'static str,
    incremental: Vec<String>,
    fresh: Vec<String>,
) -> Option<ShadowMismatch> {
    let row = incremental
        .iter()
        .zip(&fresh)
        .position(|(left, right)| left != right)
        .or_else(|| {
            (incremental.len() != fresh.len()).then(|| incremental.len().min(fresh.len()))
        })?;
    Some(ShadowMismatch {
        table,
        row: Some(row),
        incremental_rows: incremental.len(),
        fresh_rows: fresh.len(),
        incremental: incremental.get(row).cloned().unwrap_or_default(),
        fresh: fresh.get(row).cloned().unwrap_or_default(),
    })
}

/// The first difference between a retained program and a fresh one, in
/// declaration order of [`Program`]'s tables, or `None` when they are equal.
#[must_use]
pub fn first_difference(incremental: &Program, fresh: &Program) -> Option<ShadowMismatch> {
    macro_rules! tables {
        ($($field:ident),* $(,)?) => {
            $(
                if let Some(found) = compare_tables(
                    stringify!($field),
                    &incremental.$field,
                    &fresh.$field,
                ) {
                    return Some(found);
                }
            )*
        };
    }
    let sorted_compute_reads = |program: &Program| {
        let mut reads = program
            .apply_compute_reads
            .iter()
            .map(|location| format!("{location:?}"))
            .collect::<Vec<_>>();
        reads.sort();
        reads
    };
    if let Some(found) = compare(
        "apply_compute_reads",
        sorted_compute_reads(incremental),
        sorted_compute_reads(fresh),
    ) {
        return Some(found);
    }
    if incremental.runtime_configuration != fresh.runtime_configuration {
        return Some(ShadowMismatch {
            table: "runtime_configuration",
            row: None,
            incremental_rows: 1,
            fresh_rows: 1,
            incremental: format!("{:?}", incremental.runtime_configuration),
            fresh: format!("{:?}", fresh.runtime_configuration),
        });
    }
    tables!(
        development_feedback,
        reads,
        writes,
        actions,
        leaf_operations,
        static_violations,
        static_defects,
        directive_creations,
        missing_owners,
        async_reads,
        undecided_component_regions,
        unreached_sites,
        early_mention_sites,
    );
    if incremental.obligation_counts != fresh.obligation_counts {
        return Some(ShadowMismatch {
            table: "obligation_counts",
            row: None,
            incremental_rows: 1,
            fresh_rows: 1,
            incremental: format!("{:?}", incremental.obligation_counts),
            fresh: format!("{:?}", fresh.obligation_counts),
        });
    }
    // Reached only if a table was added to `Program` and not to the list
    // above: fail closed rather than call an unexamined difference equal.
    (incremental != fresh).then(|| ShadowMismatch {
        table: "<table not covered by shadow comparison>",
        row: None,
        incremental_rows: 0,
        fresh_rows: 0,
        incremental: String::new(),
        fresh: String::new(),
    })
}

/// What a shadow comparison cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShadowReport {
    pub compared: bool,
    pub mismatched: bool,
    pub fresh_build: Duration,
    pub compare: Duration,
}

/// Reports a mismatch per `mode`: panics, or logs (stderr and the optional
/// log file) and returns so the caller serves the fresh program.
pub(crate) fn report(mode: ShadowMode, mismatch: &ShadowMismatch) {
    report_message(mode, &mismatch.to_string());
}

/// [`report`] for any shadow finding, such as a base-index mismatch
/// ([`crate::base::retained`]).
pub(crate) fn report_message(mode: ShadowMode, mismatch: &str) {
    let message = format!("solid-checker: {mismatch}");
    if mode == ShadowMode::Panic {
        panic!("{message}");
    }
    eprintln!("{message}");
    if let Some(path) = std::env::var_os("SOLID_CHECKER_IR_SHADOW_LOG")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_selects_the_mode() {
        assert_eq!(ShadowMode::from_setting(None), ShadowMode::Off);
        assert_eq!(ShadowMode::from_setting(Some("0")), ShadowMode::Off);
        assert_eq!(ShadowMode::from_setting(Some("1")), ShadowMode::Serve);
        assert_eq!(ShadowMode::from_setting(Some("panic")), ShadowMode::Panic);
    }

    #[test]
    fn equal_programs_do_not_differ_and_a_changed_row_is_named() {
        let left = Program::default();
        let mut right = Program::default();
        assert_eq!(first_difference(&left, &right), None);
        right.obligation_counts.strict_reads = 1;
        let found = first_difference(&left, &right).expect("counts differ");
        assert_eq!(found.table, "obligation_counts");
    }

    #[test]
    fn a_differing_or_missing_row_is_named() {
        let rows = |items: &[&str]| {
            items
                .iter()
                .map(|item| (*item).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(compare("reads", rows(&["a", "b"]), rows(&["a", "b"])), None);
        let found = compare("reads", rows(&["a", "b"]), rows(&["a", "c"])).unwrap();
        assert_eq!((found.table, found.row), ("reads", Some(1)));
        let found = compare("reads", rows(&["a"]), rows(&["a", "c"])).unwrap();
        assert_eq!(
            (found.row, found.incremental.as_str(), found.fresh.as_str()),
            (Some(1), "", "c")
        );
        // Tables compare row by row with the same answers.
        assert_eq!(compare_tables("reads", &[1, 2], &[1, 2]), None);
        let found = compare_tables("reads", &[1, 2], &[1, 3]).unwrap();
        assert_eq!(
            (found.row, found.incremental.as_str(), found.fresh.as_str()),
            (Some(1), "2", "3")
        );
        let found = compare_tables("reads", &[1], &[1, 3]).unwrap();
        assert_eq!(
            (found.row, found.incremental_rows, found.fresh.as_str()),
            (Some(1), 1, "3")
        );
    }
}
