//! Reproducible Phase 16 measurements over receipt-issued first-party cases.
//!
//! The benchmark deliberately runs outside ordinary analysis. It retains raw
//! proof-transcript sizes only as numbers, then loads the finalized main
//! document and receipt through the same boundary as embedded bundles. Query
//! timing runs solely against accepted normalized contracts.

use std::{hint::black_box, time::Instant};

use serde_json::{Value, json};

use crate::{
    contract_document_v2::{self, SidecarDigests},
    contract_interface::load_receipt_issued_embedded_contract,
    first_party_bundles::{solid1_bundles_with_measurements, solid2_rc3_bundles_with_measurements},
};

const DEFAULT_LOAD_ITERATIONS: usize = 25;
const DEFAULT_QUERY_ITERATIONS: usize = 250;

pub fn phase16_benchmark_report() -> Result<Value, Box<dyn std::error::Error>> {
    let baseline_resident_kib = resident_kib();
    let mut bundles = solid1_bundles_with_measurements()?;
    bundles.extend(solid2_rc3_bundles_with_measurements()?);
    bundles.sort_by(|left, right| left.bundle.file_stem.cmp(&right.bundle.file_stem));

    let mut pretty_main = Vec::new();
    let mut canonical_main = Vec::new();
    let mut normalized_debug = Vec::new();
    let mut proposal = Vec::new();
    let mut plan = Vec::new();
    let mut proof = Vec::new();
    let mut receipt = Vec::new();
    let mut generation = Vec::new();
    let mut verification = Vec::new();
    let mut bytes_per_export = Vec::new();
    let mut bytes_per_operation = Vec::new();
    let mut export_count = 0usize;
    let mut operation_count = 0usize;

    for measured in &bundles {
        let bundle = &measured.bundle;
        let normalized = contract_document_v2::decode(&bundle.document)?.normalize()?;
        let canonical =
            contract_document_v2::encode(&normalized, &SidecarDigests::default(), false)?;
        let exports = normalized
            .artifact_cases()
            .iter()
            .map(|artifact| artifact.exports.len())
            .sum::<usize>();
        let operations = normalized
            .artifact_cases()
            .iter()
            .flat_map(|artifact| artifact.exports.values())
            .map(|export| export.call.operations.len())
            .sum::<usize>();

        pretty_main.push(bundle.document.len() as u64);
        canonical_main.push(canonical.len() as u64);
        normalized_debug.push(format!("{normalized:?}").len() as u64);
        proposal.push(measured.measurements.proposal_bytes as u64);
        plan.push(measured.measurements.plan_bytes as u64);
        proof.push(measured.measurements.proof_bytes as u64);
        receipt.push(bundle.receipt.len() as u64);
        generation.push(saturating_u64(measured.measurements.generation_ns));
        verification.push(saturating_u64(measured.measurements.verification_ns));
        if exports > 0 {
            bytes_per_export.push(canonical.len() as f64 / exports as f64);
        }
        if operations > 0 {
            bytes_per_operation.push(canonical.len() as f64 / operations as f64);
        }
        export_count += exports;
        operation_count += operations;
    }

    let mut load_samples = Vec::with_capacity(DEFAULT_LOAD_ITERATIONS);
    for _ in 0..DEFAULT_LOAD_ITERATIONS {
        let started = Instant::now();
        for measured in &bundles {
            let bundle = &measured.bundle;
            black_box(load_receipt_issued_embedded_contract(
                &bundle.document,
                &bundle.receipt,
            )?);
        }
        load_samples.push(saturating_u64(started.elapsed().as_nanos()));
    }

    let accepted = bundles
        .iter()
        .map(|measured| {
            load_receipt_issued_embedded_contract(
                &measured.bundle.document,
                &measured.bundle.receipt,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let query_count = accepted
        .iter()
        .map(|contract| contract.artifact_case().exports.len())
        .sum::<usize>();
    let mut query_samples = Vec::with_capacity(DEFAULT_QUERY_ITERATIONS);
    for _ in 0..DEFAULT_QUERY_ITERATIONS {
        let started = Instant::now();
        for contract in &accepted {
            for name in contract.artifact_case().exports.keys() {
                black_box(contract.export(name).expect("indexed export must resolve"));
            }
        }
        query_samples.push(if query_count == 0 {
            0
        } else {
            saturating_u64(started.elapsed().as_nanos()) / query_count as u64
        });
    }
    let loaded_resident_kib = resident_kib();

    Ok(json!({
        "schemaVersion": 1,
        "documentKind": "solid-checker-phase16-accepted-corpus-benchmark",
        "corpus": {
            "receiptIssuedArtifactCases": bundles.len(),
            "exports": export_count,
            "operations": operation_count,
            "proofPolicy": "repository-owned checked authorities only"
        },
        "compactness": {
            "prettyMainBytes": distribution(&pretty_main),
            "canonicalMainBytes": distribution(&canonical_main),
            "normalizedSemanticDebugBytes": distribution(&normalized_debug),
            "proposalBytes": distribution(&proposal),
            "proposalPlanBytes": distribution(&plan),
            "proofEvidenceBytes": distribution(&proof),
            "acceptanceReceiptBytes": distribution(&receipt),
            "canonicalBytesPerExport": float_distribution(&bytes_per_export),
            "canonicalBytesPerOperation": float_distribution(&bytes_per_operation),
            "rawEvidenceRetainedByOrdinaryAnalysis": 0
        },
        "performance": {
            "proposalAndProofInputGenerationNs": distribution(&generation),
            "proofVerificationAndReceiptNs": distribution(&verification),
            "acceptedCorpusLoadNs": distribution(&load_samples),
            "normalizedQueryNsPerExport": distribution(&query_samples),
            "loadIterations": DEFAULT_LOAD_ITERATIONS,
            "queryIterations": DEFAULT_QUERY_ITERATIONS,
            "memory": {
                "method": "getrusage-peak-rss",
                "scope": "whole benchmark process; includes checked-corpus construction and accepted loading; not a retained-heap measurement",
                "baselinePeakResidentKiB": baseline_resident_kib,
                "postLoadPeakResidentKiB": loaded_resident_kib,
                "observedPeakDeltaKiB": resident_delta(baseline_resident_kib, loaded_resident_kib)
            }
        },
        "ordinaryAnalysis": {
            "input": "AcceptedContractIndex / receipt-validated normalized semantics",
            "rawSidecarBytes": 0,
            "packageCodeExecution": false,
            "networkAccess": false,
            "queryFileReads": false
        }
    }))
}

fn distribution(values: &[u64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    json!({
        "count": sorted.len(),
        "p50": percentile(&sorted, 50),
        "p95": percentile(&sorted, 95),
        "max": sorted.last().copied()
    })
}

fn float_distribution(values: &[f64]) -> Value {
    let rounded = values
        .iter()
        .map(|value| (value * 100.0).round() as u64)
        .collect::<Vec<_>>();
    let mut value = distribution(&rounded);
    for field in ["p50", "p95", "max"] {
        if let Some(number) = value[field].as_u64() {
            value[field] = json!(number as f64 / 100.0);
        }
    }
    value
}

fn percentile(sorted: &[u64], percentage: usize) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let index = (sorted.len() * percentage).div_ceil(100).saturating_sub(1);
    sorted.get(index).copied()
}

fn saturating_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn resident_delta(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    Some(after?.saturating_sub(before?))
}

#[cfg(unix)]
fn resident_kib() -> Option<u64> {
    // SAFETY: `usage` points to a valid writable `rusage`, and RUSAGE_SELF
    // requires no caller-owned lifetime. The OS initializes the full value.
    let usage = unsafe {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        usage.assume_init()
    };
    let peak = u64::try_from(usage.ru_maxrss).ok()?;
    if cfg!(target_os = "macos") {
        Some(peak / 1024)
    } else {
        Some(peak)
    }
}

#[cfg(not(unix))]
fn resident_kib() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_queries_are_closed_over_normalized_semantics_after_raw_inputs_are_dropped() {
        let mut bundles = solid1_bundles_with_measurements().unwrap();
        bundles.extend(solid2_rc3_bundles_with_measurements().unwrap());
        let accepted = bundles
            .iter()
            .map(|measured| {
                load_receipt_issued_embedded_contract(
                    &measured.bundle.document,
                    &measured.bundle.receipt,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let names = accepted
            .iter()
            .map(|contract| {
                contract
                    .artifact_case()
                    .exports
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // Main bytes, receipts, and any raw proof material belong to the
        // acquisition scope. Analyzer queries retain only accepted semantics.
        drop(bundles);
        for (contract, names) in accepted.iter().zip(names) {
            for name in names {
                assert!(contract.export(&name).is_some());
            }
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn resident_memory_is_explicitly_unavailable_without_getrusage() {
        assert_eq!(resident_kib(), None);
    }
}
