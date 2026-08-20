//! Validity model for daemon snapshots.
//!
//! Filesystem discovery and hashing stay with the daemon adapter. This module
//! owns the invariant that a cached answer is reusable only when every input
//! that influenced it is identical.

use std::{path::PathBuf, sync::Arc};

pub(crate) type ContractFile = (PathBuf, [u8; 32]);
pub(crate) type CachedSnapshot = (Arc<str>, Arc<[u8]>);

pub(crate) struct CachedAnswer {
    pub(crate) generation: u64,
    pub(crate) explicit: Vec<String>,
    pub(crate) modules: Vec<String>,
    pub(crate) contract_files: Vec<ContractFile>,
    pub(crate) presets: Vec<String>,
    pub(crate) enable_rules: Vec<String>,
    pub(crate) status: Arc<str>,
    pub(crate) body: Arc<[u8]>,
}

impl CachedAnswer {
    pub(crate) fn snapshot_if_current(
        &self,
        generation: u64,
        explicit: &[String],
        contract_files: &[ContractFile],
        presets: &[String],
        enable_rules: &[String],
    ) -> Option<CachedSnapshot> {
        (self.generation == generation
            && self.explicit == explicit
            && self.contract_files == contract_files
            && self.presets == presets
            && self.enable_rules == enable_rules)
            .then(|| (Arc::clone(&self.status), Arc::clone(&self.body)))
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::CachedAnswer;

    fn cached() -> CachedAnswer {
        CachedAnswer {
            generation: 3,
            explicit: vec!["explicit.json".into()],
            modules: vec!["solid-js".into()],
            contract_files: vec![(PathBuf::from("solid-reactivity.json"), [7; 32])],
            presets: vec!["preferences".into()],
            enable_rules: vec!["prefer-show".into()],
            status: Arc::from("certified"),
            body: Arc::from(&b"snapshot"[..]),
        }
    }

    #[test]
    fn unchanged_snapshot_inputs_reuse_the_answer() {
        let cached = cached();
        let contracts = cached.contract_files.clone();
        assert_eq!(
            cached.snapshot_if_current(
                3,
                &cached.explicit,
                &contracts,
                &cached.presets,
                &cached.enable_rules,
            ),
            Some((Arc::from("certified"), Arc::from(&b"snapshot"[..])))
        );
    }

    #[test]
    fn preset_change_misses_the_answer_cache() {
        let cached = cached();
        assert!(
            cached
                .snapshot_if_current(
                    cached.generation,
                    &cached.explicit,
                    &cached.contract_files,
                    &[],
                    &cached.enable_rules,
                )
                .is_none()
        );
    }

    #[test]
    fn enabled_rule_change_misses_the_answer_cache() {
        let cached = cached();
        assert!(
            cached
                .snapshot_if_current(
                    cached.generation,
                    &cached.explicit,
                    &cached.contract_files,
                    &cached.presets,
                    &[],
                )
                .is_none()
        );
    }
}
