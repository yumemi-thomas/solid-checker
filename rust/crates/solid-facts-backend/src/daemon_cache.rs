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
    /// The directories whose catalogs could apply to an analysed file when
    /// this answer was computed (`nested_catalog_candidates`): the
    /// `contract_files` comparison re-reads each of them, the way `modules`
    /// names the packages whose contracts it re-reads.
    pub(crate) catalog_scopes: Vec<PathBuf>,
    /// The `(install directory, module)` lookups artifact admission made for
    /// imports some file reaches outside the project directory's own installs
    /// (`importer_admission_inputs`): the `contract_files` comparison re-reads
    /// each one's manifest and lockfiles.
    pub(crate) install_lookups: Vec<(PathBuf, String)>,
    pub(crate) contract_files: Vec<ContractFile>,
    /// The inference closure observed before analysis, including absent probes
    /// and directory/symlink identities. Rehash it on reuse; do not rediscover
    /// or reparse the application for an unchanged answer.
    pub(crate) inference_inputs: Vec<ContractFile>,
    pub(crate) presets: Vec<String>,
    pub(crate) enable_rules: Vec<String>,
    pub(crate) runtime: RuntimeEnvironment,
    pub(crate) status: Arc<str>,
    pub(crate) body: Arc<[u8]>,
    /// The withheld-catalog notice the answer was computed with. It is a
    /// function of the same inputs as the snapshot, so it is reused with it.
    pub(crate) notice: Arc<str>,
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
            catalog_scopes: vec![PathBuf::from("packages/a")],
            install_lookups: vec![(PathBuf::from("packages/a"), "pkg".into())],
            contract_files: vec![(PathBuf::from("solid-reactivity.json"), [7; 32])],
            inference_inputs: Vec::new(),
            presets: vec!["preferences".into()],
            enable_rules: vec!["prefer-show".into()],
            runtime: RuntimeEnvironment::default(),
            status: Arc::from("certified"),
            body: Arc::from(&b"snapshot"[..]),
            notice: Arc::from(""),
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
