//! Validity model for daemon snapshots.
//!
//! Filesystem discovery and hashing stay with the daemon adapter. This module
//! owns the invariant that a cached answer is reusable only when every input
//! that influenced it is identical.

use std::{path::PathBuf, sync::Arc};

use solid_reactive_ir::RuntimeEnvironment;

pub(crate) type InputFile = (PathBuf, [u8; 32]);
pub(crate) type CachedSnapshot = (Arc<str>, Arc<[u8]>);

pub(crate) struct CachedAnswer {
    pub(crate) generation: u64,
    /// The imported package roots whose manifests `input_files` re-reads.
    pub(crate) modules: Vec<String>,
    /// The installed router directories the analysis resolved to, whose bytes
    /// `input_files` re-hashes (ADR 0324).
    pub(crate) router_roots: Vec<PathBuf>,
    pub(crate) input_files: Vec<InputFile>,
    /// The inference closure observed before analysis, including absent probes
    /// and directory/symlink identities. Rehash it on reuse; do not rediscover
    /// or reparse the application for an unchanged answer.
    pub(crate) inference_inputs: Vec<InputFile>,
    pub(crate) presets: Vec<String>,
    pub(crate) enable_rules: Vec<String>,
    pub(crate) host_suppressions: Vec<String>,
    pub(crate) runtime: RuntimeEnvironment,
    /// `--feedback-facts`: the answer carries the development feedback
    /// models (ADR 0543). A daemon serves one mode, and this keeps an answer
    /// of the other from being reused even so.
    pub(crate) feedback_facts: bool,
    pub(crate) status: Arc<str>,
    pub(crate) body: Arc<[u8]>,
    pub(crate) index: Arc<crate::json_output::FindingIndex>,
    /// The notice the answer was computed with. It is a function of the same
    /// inputs as the snapshot, so it is reused with it.
    pub(crate) notice: Arc<str>,
}

impl CachedAnswer {
    // One argument per input the answer is reused under, so a new input
    // cannot be left out of a call.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn snapshot_if_current(
        &self,
        generation: u64,
        input_files: &[InputFile],
        presets: &[String],
        enable_rules: &[String],
        host_suppressions: &[String],
        runtime: &RuntimeEnvironment,
        feedback_facts: bool,
    ) -> Option<CachedSnapshot> {
        (self.generation == generation
            && self.input_files == input_files
            && self.presets == presets
            && self.enable_rules == enable_rules
            && self.host_suppressions == host_suppressions
            && self.runtime == *runtime
            && self.feedback_facts == feedback_facts)
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
            modules: vec!["solid-js".into()],
            router_roots: Vec::new(),
            input_files: vec![(PathBuf::from("package.json"), [7; 32])],
            inference_inputs: Vec::new(),
            presets: vec!["preferences".into()],
            enable_rules: vec!["prefer-show".into()],
            host_suppressions: Vec::new(),
            runtime: RuntimeEnvironment::default(),
            feedback_facts: false,
            status: Arc::from("certified"),
            body: Arc::from(&b"snapshot"[..]),
            index: Arc::default(),
            notice: Arc::from(""),
        }
    }

    #[test]
    fn every_snapshot_input_participates_in_reuse() {
        let cached = cached();
        let inputs = cached.input_files.clone();
        assert_eq!(
            cached.snapshot_if_current(
                3,
                &inputs,
                &cached.presets,
                &cached.enable_rules,
                &cached.host_suppressions,
                &cached.runtime,
                cached.feedback_facts,
            ),
            Some((Arc::from("certified"), Arc::from(&b"snapshot"[..])))
        );
        let changed = [
            cached.snapshot_if_current(
                cached.generation + 1,
                &cached.input_files,
                &cached.presets,
                &cached.enable_rules,
                &cached.host_suppressions,
                &cached.runtime,
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &[],
                &cached.presets,
                &cached.enable_rules,
                &cached.host_suppressions,
                &cached.runtime,
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.input_files,
                &[],
                &cached.enable_rules,
                &cached.host_suppressions,
                &cached.runtime,
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.input_files,
                &cached.presets,
                &[],
                &cached.host_suppressions,
                &cached.runtime,
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.input_files,
                &cached.presets,
                &cached.enable_rules,
                &["strict-read-untracked".into()],
                &cached.runtime,
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.input_files,
                &cached.presets,
                &cached.enable_rules,
                &cached.host_suppressions,
                &RuntimeEnvironment {
                    target: Some(RuntimeTarget::Browser),
                    ..RuntimeEnvironment::default()
                },
                cached.feedback_facts,
            ),
            cached.snapshot_if_current(
                cached.generation,
                &cached.input_files,
                &cached.presets,
                &cached.enable_rules,
                &cached.host_suppressions,
                &cached.runtime,
                !cached.feedback_facts,
            ),
        ];
        assert!(changed.into_iter().all(|snapshot| snapshot.is_none()));
    }
}
