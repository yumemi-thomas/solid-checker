use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
struct Options {
    checker: PathBuf,
    typefacts: PathBuf,
    project: PathBuf,
    iterations: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliTimings {
    sidecar_spawn_ns: u64,
    sources_fetch_ns: u64,
    sources_bytes: u64,
    #[serde(default)]
    sources_wire_bytes: u64,
    source_setup_ns: u64,
    source_analysis_ns: u64,
    type_facts_ns: u64,
    facts_total_ns: u64,
    ir_ns: u64,
    solve_and_snapshot_ns: u64,
    total_ns: u64,
    #[serde(default)]
    wall_ns: u64,
    #[serde(default)]
    process_overhead_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Distribution {
    samples_ns: Vec<u64>,
    min_ns: u64,
    median_ns: u64,
    p95_ns: u64,
    max_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StageSummary {
    wall: Distribution,
    measured_cli_total: Distribution,
    process_overhead: Distribution,
    sidecar_spawn: Distribution,
    sources_fetch: Distribution,
    sources_payload_bytes: Distribution,
    sources_wire_bytes: Distribution,
    source_analysis: Distribution,
    type_facts: Distribution,
    facts_total: Distribution,
    reactive_ir: Distribution,
    solve_and_snapshot: Distribution,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("solid-checker-cold-start-bench: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args()?;
    let checker = options.checker.canonicalize()?;
    let typefacts = options.typefacts.canonicalize()?;
    let project = options.project.canonicalize()?;

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        samples.push(measure_once(&checker, &typefacts, &project)?);
    }
    let first_observed = samples.remove(0);
    let summary = summarize(&samples);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "benchmark": "fresh-process-cold-start",
            "coldStartDefinition": "A new Rust checker and TypeScript-Go sidecar process per sample. Only the first observed sample may include cold executable/filesystem pages; later samples are process-cold with warm OS caches.",
            "project": project,
            "checker": checker,
            "typefacts": typefacts,
            "platform": {
                "os": env::consts::OS,
                "arch": env::consts::ARCH,
            },
            "iterations": options.iterations,
            "firstObserved": first_observed,
            "subsequentFreshProcesses": summary,
        }))?
    );
    Ok(())
}

fn measure_once(
    checker: &Path,
    typefacts: &Path,
    project: &Path,
) -> Result<CliTimings, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let output = Command::new(checker)
        .arg("--format=json")
        .arg("--project")
        .arg(project)
        .arg("--typefacts")
        .arg(typefacts)
        .env("SOLID_CHECK_TIMINGS", "1")
        .env_remove("SOLID_CHECK_DAEMON")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    let wall_ns = nanos(started.elapsed());
    if !output.status.success() {
        return Err(format!(
            "checker exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let stderr = String::from_utf8(output.stderr)?;
    let timing_line = stderr
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or_else(|| format!("checker emitted no timing JSON on stderr: {stderr:?}"))?;
    let mut timings: CliTimings = serde_json::from_str(timing_line)?;
    timings.wall_ns = wall_ns;
    timings.process_overhead_ns = wall_ns.saturating_sub(timings.total_ns);
    Ok(timings)
}

fn summarize(samples: &[CliTimings]) -> StageSummary {
    StageSummary {
        wall: distribution(samples.iter().map(|sample| sample.wall_ns)),
        measured_cli_total: distribution(samples.iter().map(|sample| sample.total_ns)),
        process_overhead: distribution(samples.iter().map(|sample| sample.process_overhead_ns)),
        sidecar_spawn: distribution(samples.iter().map(|sample| sample.sidecar_spawn_ns)),
        sources_fetch: distribution(samples.iter().map(|sample| sample.sources_fetch_ns)),
        sources_payload_bytes: distribution(samples.iter().map(|sample| sample.sources_bytes)),
        sources_wire_bytes: distribution(samples.iter().map(|sample| sample.sources_wire_bytes)),
        source_analysis: distribution(samples.iter().map(|sample| sample.source_analysis_ns)),
        type_facts: distribution(samples.iter().map(|sample| sample.type_facts_ns)),
        facts_total: distribution(samples.iter().map(|sample| sample.facts_total_ns)),
        reactive_ir: distribution(samples.iter().map(|sample| sample.ir_ns)),
        solve_and_snapshot: distribution(samples.iter().map(|sample| sample.solve_and_snapshot_ns)),
    }
}

fn distribution(samples: impl Iterator<Item = u64>) -> Distribution {
    let mut samples_ns = samples.collect::<Vec<_>>();
    samples_ns.sort_unstable();
    Distribution {
        min_ns: samples_ns.first().copied().unwrap_or(0),
        median_ns: percentile(&samples_ns, 50),
        p95_ns: percentile(&samples_ns, 95),
        max_ns: samples_ns.last().copied().unwrap_or(0),
        samples_ns,
    }
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    samples[(samples.len() - 1) * percentile / 100]
}

fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn parse_args() -> Result<Options, Box<dyn std::error::Error>> {
    let mut checker = PathBuf::from("rust/target/release/solid-checker-rust");
    let mut typefacts = env::var_os("SOLID_TYPEFACTS_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("bin/solid-typefacts"));
    let mut project = PathBuf::from("fixtures/engine/eslint-reactivity-v2/tsconfig.json");
    let mut iterations = 10_usize;
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        let value = |index: &mut usize| -> Result<&str, Box<dyn std::error::Error>> {
            *index += 1;
            arguments
                .get(*index)
                .map(String::as_str)
                .ok_or_else(|| format!("{argument} requires a value").into())
        };
        match argument.as_str() {
            "--checker" => checker = value(&mut index)?.into(),
            "--typefacts" => typefacts = value(&mut index)?.into(),
            "--project" => project = value(&mut index)?.into(),
            "--iterations" => iterations = value(&mut index)?.parse()?,
            "-h" | "--help" => {
                println!(
                    "Usage: solid-checker-cold-start-bench [OPTIONS]\n\n\
                     --checker <PATH>       Release solid-checker-rust executable\n\
                     --typefacts <PATH>     TypeFacts service executable\n\
                     --project <PATH>       TypeScript project\n\
                     --iterations <COUNT>   Fresh process samples, including the first observation (default: 10)"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown option {argument:?}").into()),
        }
        index += 1;
    }
    if iterations < 2 {
        return Err("--iterations must be at least 2".into());
    }
    Ok(Options {
        checker,
        typefacts,
        project,
        iterations,
    })
}

#[cfg(test)]
mod tests {
    use super::{distribution, percentile};

    #[test]
    fn nearest_rank_summary_is_deterministic() {
        let distribution = distribution([50, 10, 40, 20, 30].into_iter());
        assert_eq!(distribution.samples_ns, vec![10, 20, 30, 40, 50]);
        assert_eq!(distribution.min_ns, 10);
        assert_eq!(distribution.median_ns, 30);
        assert_eq!(distribution.p95_ns, 40);
        assert_eq!(distribution.max_ns, 50);
        assert_eq!(percentile(&[], 50), 0);
    }
}
