//! Validity model for daemon snapshots.
//!
//! Filesystem discovery and hashing stay with the daemon adapter. This module
//! owns the invariant that a cached answer is reusable only when every input
//! that influenced it is identical.

use std::{path::PathBuf, sync::Arc};

use solid_reactive_ir::RuntimeEnvironment;

pub(crate) type ContractFile = (PathBuf, [u8; 32]);
pub(crate) type CachedSnapshot = (Arc<str>, Arc<[u8]>);

pub(crate) struct CachedAnswer {
    pub(crate) generation: u64,
    pub(crate) explicit: Vec<String>,
    pub(crate) modules: Vec<String>,
    pub(crate) contract_files: Vec<ContractFile>,
    pub(crate) presets: Vec<String>,
    pub(crate) enable_rules: Vec<String>,
    pub(crate) runtime: RuntimeEnvironment,
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
        runtime: &RuntimeEnvironment,
    ) -> Option<CachedSnapshot> {
        (self.generation == generation
            && self.explicit == explicit
            && self.contract_files == contract_files
            && self.presets == presets
            && self.enable_rules == enable_rules
            && self.runtime == *runtime)
            .then(|| (Arc::clone(&self.status), Arc::clone(&self.body)))
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::CachedAnswer;
    use solid_reactive_ir::{RuntimeEnvironment, RuntimeTarget};

    fn cached() -> CachedAnswer {
        CachedAnswer {
            generation: 3,
            explicit: vec!["explicit.json".into()],
            modules: vec!["solid-js".into()],
            contract_files: vec![(PathBuf::from("solid-reactivity.json"), [7; 32])],
            presets: vec!["preferences".into()],
            enable_rules: vec!["prefer-show".into()],
            runtime: RuntimeEnvironment::default(),
            status: Arc::from("certified"),
            body: Arc::from(&b"snapshot"[..]),
        }
    }

    #[test]
    fn every_snapshot_input_participates_in_reuse() {
        let cached = cached();
        let contracts = cached.contract_files.clone();
        assert_eq!(
            cached.snapshot_if_current(
                3,
                &cached.explicit,
                &contracts,
                &cached.presets,
                &cached.enable_rules,
                &cached.runtime,
            ),
            Some((Arc::from("certified"), Arc::from(&b"snapshot"[..])))
        );
        let changed = [
            cached.snapshot_if_current(
                cached.generation + 1,
                &cached.explicit,
                &cached.contract_files,
                &cached.presets,
                &cached.enable_rules,
                &cached.runtime,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &[],
                &cached.contract_files,
                &cached.presets,
                &cached.enable_rules,
                &cached.runtime,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.explicit,
                &[],
                &cached.presets,
                &cached.enable_rules,
                &cached.runtime,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.explicit,
                &cached.contract_files,
                &[],
                &cached.enable_rules,
                &cached.runtime,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.explicit,
                &cached.contract_files,
                &cached.presets,
                &[],
                &cached.runtime,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.explicit,
                &cached.contract_files,
                &cached.presets,
                &cached.enable_rules,
                &RuntimeEnvironment {
                    target: Some(RuntimeTarget::Browser),
                    ..RuntimeEnvironment::default()
                },
            ),
        ];
        assert!(changed.into_iter().all(|snapshot| snapshot.is_none()));
    }
}
