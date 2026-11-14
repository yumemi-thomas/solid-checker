// One linked harness for the process/session integration modules that do not
// form one of the named focused verification boundaries. Keeping the source in
// its existing files preserves narrow ownership while avoiding eleven copies
// of the backend and compiler dependency graph on disk.
#[path = "support/process.rs"]
mod process_support;
// ADR 0950: the default build's half of the what-if tests (a what-if build is tested by its own target, `what_if_process`).
#[cfg(not(feature = "what-if"))]
#[path = "support/what_if.rs"]
mod what_if_support;

#[path = "accessor_provenance_process.rs"]
mod accessor_provenance_process;
#[path = "after_await_accounting_process.rs"]
mod after_await_accounting_process;
#[path = "attestation_kept_process.rs"]
mod attestation_kept_process;

#[path = "bookkeeping_process.rs"]
mod bookkeeping_process;
#[path = "callback_props_reads_process.rs"]
mod callback_props_reads_process;
#[path = "cli_process.rs"]
mod cli_process;
#[path = "complexity_ladder_process.rs"]
mod complexity_ladder_process;
#[path = "cross_file_callbacks_process.rs"]
mod cross_file_callbacks_process;
#[path = "cross_file_digest_process.rs"]
mod cross_file_digest_process;
#[path = "daemon_residue_process.rs"]
mod daemon_residue_process;
#[path = "dispatch_veto_reads_process.rs"]
mod dispatch_veto_reads_process;
#[path = "forwarding_refresh_process.rs"]
mod forwarding_refresh_process;
#[path = "harness_process.rs"]
mod harness_process;
#[path = "host_inputs_unobservable_process.rs"]
mod host_inputs_unobservable_process;
#[path = "idle_shadow_process.rs"]
mod idle_shadow_process;
#[path = "imported_const_process.rs"]
mod imported_const_process;
#[path = "incremental_staleness_process.rs"]
mod incremental_staleness_process;
#[path = "interproc_fixed_points_process.rs"]
mod interproc_fixed_points_process;
#[path = "invocation_flow_process.rs"]
mod invocation_flow_process;
#[path = "owner_flow_process.rs"]
mod owner_flow_process;
#[path = "owner_parity_process.rs"]
mod owner_parity_process;
#[path = "path_independence_process.rs"]
mod path_independence_process;
#[path = "proof_log_process.rs"]
mod proof_log_process;
#[path = "props_reactivity_process.rs"]
mod props_reactivity_process;
#[path = "reachability_parity_process.rs"]
mod reachability_parity_process;
#[path = "role_reads_process.rs"]
mod role_reads_process;
#[path = "router_params_process.rs"]
mod router_params_process;
#[path = "rule_quality_process.rs"]
mod rule_quality_process;
#[path = "sessions_process.rs"]
mod sessions_process;
#[path = "site_accounting_process.rs"]
mod site_accounting_process;
#[path = "site_reach_reads_process.rs"]
mod site_reach_reads_process;
#[path = "transport_process.rs"]
mod transport_process;
#[path = "warm_resolution_process.rs"]
mod warm_resolution_process;
#[cfg(not(feature = "what-if"))]
#[path = "what_if_default_process.rs"]
mod what_if_default_process;
