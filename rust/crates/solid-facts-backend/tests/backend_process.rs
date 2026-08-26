// One linked harness for the process/session integration modules that do not
// form one of the named focused verification boundaries. Keeping the source in
// its existing files preserves narrow ownership while avoiding eleven copies
// of the backend and compiler dependency graph on disk.
#[path = "support/process.rs"]
mod process_support;

#[path = "cli_process.rs"]
mod cli_process;
#[path = "cross_file_callbacks_process.rs"]
mod cross_file_callbacks_process;
#[path = "cross_file_digest_process.rs"]
mod cross_file_digest_process;
#[path = "harness_process.rs"]
mod harness_process;
#[path = "json_import_contract_process.rs"]
mod json_import_contract_process;
#[path = "owner_parity_process.rs"]
mod owner_parity_process;
#[path = "props_reactivity_process.rs"]
mod props_reactivity_process;
#[path = "reachability_parity_process.rs"]
mod reachability_parity_process;
#[path = "rule_quality_process.rs"]
mod rule_quality_process;
#[path = "sessions_process.rs"]
mod sessions_process;
#[path = "transport_process.rs"]
mod transport_process;
