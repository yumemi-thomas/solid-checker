// One linked harness for the process/session integration modules that do not
// form one of the named focused verification boundaries. Keeping the source in
// its existing files preserves narrow ownership while avoiding eleven copies
// of the backend and compiler dependency graph on disk.
#[path = "support/process.rs"]
mod process_support;

#[path = "accessor_provenance_process.rs"]
mod accessor_provenance_process;

#[path = "callback_props_reads_process.rs"]
mod callback_props_reads_process;
#[path = "cli_process.rs"]
mod cli_process;
#[path = "cross_file_callbacks_process.rs"]
mod cross_file_callbacks_process;
#[path = "cross_file_digest_process.rs"]
mod cross_file_digest_process;
#[path = "harness_process.rs"]
mod harness_process;
#[path = "idle_shadow_process.rs"]
mod idle_shadow_process;
#[path = "imported_const_process.rs"]
mod imported_const_process;
#[path = "incremental_staleness_process.rs"]
mod incremental_staleness_process;
#[path = "invocation_flow_process.rs"]
mod invocation_flow_process;
#[path = "owner_flow_process.rs"]
mod owner_flow_process;
#[path = "owner_parity_process.rs"]
mod owner_parity_process;
#[path = "props_reactivity_process.rs"]
mod props_reactivity_process;
#[path = "reachability_parity_process.rs"]
mod reachability_parity_process;
#[path = "router_params_process.rs"]
mod router_params_process;
#[path = "rule_quality_process.rs"]
mod rule_quality_process;
#[path = "sessions_process.rs"]
mod sessions_process;
#[path = "transport_process.rs"]
mod transport_process;
