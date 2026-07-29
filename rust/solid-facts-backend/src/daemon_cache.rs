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
    pub(crate) status: Arc<str>,
    pub(crate) body: Arc<[u8]>,
}

impl CachedAnswer {
    pub(crate) fn snapshot_if_current(
        &self,
        generation: u64,
        explicit: &[String],
        contract_files: &[ContractFile],
    ) -> Option<CachedSnapshot> {
        (self.generation == generation
            && self.explicit == explicit
            && self.contract_files == contract_files)
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
            status: Arc::from("certified"),
            body: Arc::from(&b"snapshot"[..]),
        }
    }

    #[test]
    fn every_snapshot_input_participates_in_reuse() {
        let cached = cached();
        let contracts = cached.contract_files.clone();
        assert_eq!(
            cached.snapshot_if_current(3, &cached.explicit, &contracts),
            Some((Arc::from("certified"), Arc::from(&b"snapshot"[..])))
        );
        assert!(
            cached
                .snapshot_if_current(4, &cached.explicit, &contracts)
                .is_none()
        );
        assert!(cached.snapshot_if_current(3, &[], &contracts).is_none());
        assert!(
            cached
                .snapshot_if_current(3, &cached.explicit, &[])
                .is_none()
        );
    }
}
