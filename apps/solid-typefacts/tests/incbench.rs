//! Edit latency of one producer through the client crate's `Session`, per edit
//! kind. Ignored (it measures):
//!
//! ```text
//! TSRS_BENCH_BIN=<producer> TSRS_BENCH_PROJECT=<tsconfig.json> \
//!   [TSRS_BENCH_FILE=<path substring>] [TSRS_BENCH_RUNS=7] \
//!   cargo test --release --test incbench -- --ignored --nocapture
//! ```
//!
//! Each row is the median over runs of: `update` (ms), then `analyze` (ms) over
//! the differential demand set (symbol, referenceSpace, references on every
//! 4th span, structuralAccessor on every 9th), re-derived from the current
//! sources outside the timer.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use typefacts::v3::{EntityDemand, FileChange, SourceFile};
use typefacts::{AnalysisDemand, Location, Producer, Session};

fn is_word(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

/// Every word and every dotted chain of words in `source`, as byte spans. A
/// chain is demanded as a whole so the covering claim sees `a.b` (a span whose
/// first token is not the whole span) as well as bare identifiers.
fn spans(source: &str) -> Vec<(usize, usize)> {
    const LIMIT: usize = 1500;
    let bytes = source.as_bytes();
    let mut found: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' || bytes[index] == b'$') {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = start;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        found.insert((start, end));
        let mut chain_end = end;
        while bytes.get(chain_end) == Some(&b'.')
            && bytes.get(chain_end + 1).is_some_and(|byte| is_word(*byte))
        {
            chain_end += 1;
            while chain_end < bytes.len() && is_word(bytes[chain_end]) {
                chain_end += 1;
            }
        }
        if chain_end != end {
            found.insert((start, chain_end));
        }
        index = end;
    }
    let all: Vec<(usize, usize)> = found.into_iter().collect();
    if all.len() <= LIMIT {
        return all;
    }
    let stride = all.len().div_ceil(LIMIT);
    all.into_iter()
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .map(|(_, span)| span)
        .collect()
}

fn demands_for(files: &[SourceFile]) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    for file in files {
        if file.source.len() > 60_000 {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&file.source) else {
            continue;
        };
        for (index, (start, end)) in spans(text).into_iter().enumerate() {
            demands.push(EntityDemand {
                location: Location {
                    path: file.path.as_str().into(),
                    start_byte: start as u64,
                    end_byte: end as u64,
                },
                symbol: true,
                reference_space: true,
                references: index % 4 == 0,
                structural_accessor: index % 9 == 0,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

fn median(mut values: Vec<Duration>) -> f64 {
    values.sort();
    values[values.len() / 2].as_secs_f64() * 1000.0
}

#[test]
#[ignore = "measures; run explicitly"]
fn edit_latency_per_kind() {
    let binary = PathBuf::from(std::env::var("TSRS_BENCH_BIN").expect("TSRS_BENCH_BIN"));
    let config = PathBuf::from(std::env::var("TSRS_BENCH_PROJECT").expect("TSRS_BENCH_PROJECT"))
        .canonicalize()
        .unwrap();
    let runs: usize = std::env::var("TSRS_BENCH_RUNS").map_or(7, |v| v.parse().unwrap());
    let project = config.to_string_lossy().into_owned();
    let mut session =
        Session::open(Producer::at(&binary), &project, Vec::<FileChange>::new()).unwrap();
    let mut sources = session.configured_sources().unwrap();
    let analyze = |session: &mut Session, sources: &[SourceFile]| {
        let demands = demands_for(sources);
        let started = Instant::now();
        let table = session
            .analyze(&AnalysisDemand { entities: demands })
            .unwrap();
        (started.elapsed(), table.entities().count())
    };
    // The edited file: named, or the project source most other sources import.
    let candidates: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            !f.path.contains("/node_modules/")
                && !f.path.ends_with(".d.ts")
                && f.source.len() < 60_000
        })
        .map(|(i, _)| i)
        .collect();
    let target = match std::env::var("TSRS_BENCH_FILE") {
        Ok(part) => *candidates
            .iter()
            .find(|i| sources[**i].path.contains(&part))
            .expect("file"),
        Err(_) => *candidates
            .iter()
            .max_by_key(|i| {
                let stem = sources[**i]
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .split('.')
                    .next()
                    .unwrap()
                    .to_owned();
                let needle = format!("/{stem}\"");
                sources
                    .iter()
                    .filter(|f| String::from_utf8_lossy(&f.source).contains(&needle))
                    .count()
            })
            .unwrap(),
    };
    let target_path = sources[target].path.clone();
    let original = sources[target].source.clone();
    let config_text = std::fs::read(&config).unwrap();
    println!(
        "project {project}\nbinary {}\nsources {}  edited {}",
        binary.display(),
        sources.len(),
        target_path
    );
    let (cold, rows) = analyze(&mut session, &sources);
    println!(
        "{:<22} analyze {:>9.1} ms  ({rows} rows)",
        "cold",
        cold.as_secs_f64() * 1000.0
    );

    let mut version = 0_u64;
    type Edit = fn(&[u8], u64) -> Vec<u8>;
    let kinds: [(&str, Edit); 4] = [
        ("body (comment at end)", |src, n| {
            let mut v = src.to_vec();
            v.extend_from_slice(format!("\n// edit {n}\n").as_bytes());
            v
        }),
        ("prefix (spans shift)", |src, n| {
            let mut v = format!("// edit {n}\n").into_bytes();
            v.extend_from_slice(src);
            v
        }),
        ("export added", |src, n| {
            let mut v = src.to_vec();
            v.extend_from_slice(format!("\nexport const __inc{n} = {n};\n").as_bytes());
            v
        }),
        ("type edit (alias added)", |src, n| {
            let mut v = src.to_vec();
            v.extend_from_slice(format!("\nexport type __IncT{n} = {{ a: {n} }};\n").as_bytes());
            v
        }),
    ];
    for (name, make) in kinds {
        let (mut updates, mut analyses) = (Vec::new(), Vec::new());
        for run in 0..runs + 1 {
            version += 1;
            let edited = make(&original, version);
            sources[target].source.clone_from(&edited);
            let started = Instant::now();
            session
                .update([FileChange {
                    path: target_path.clone(),
                    source: edited,
                    deleted: false,
                    version,
                }])
                .unwrap();
            let update = started.elapsed();
            let (analysis, _) = analyze(&mut session, &sources);
            if run > 0 {
                updates.push(update);
                analyses.push(analysis);
            }
        }
        println!(
            "{name:<22} update {:>8.1} ms  analyze {:>9.1} ms",
            median(updates),
            median(analyses)
        );
    }
    // restore, then a tsconfig edit
    version += 1;
    sources[target].source.clone_from(&original);
    session
        .update([FileChange {
            path: target_path.clone(),
            source: original.clone(),
            deleted: false,
            version,
        }])
        .unwrap();
    analyze(&mut session, &sources);
    let (mut updates, mut analyses) = (Vec::new(), Vec::new());
    for run in 0..runs + 1 {
        version += 1;
        let mut edited = config_text.clone();
        edited.extend_from_slice(format!("\n// edit {version}\n").as_bytes());
        let started = Instant::now();
        session
            .update([FileChange {
                path: config.to_string_lossy().into_owned(),
                source: edited,
                deleted: false,
                version,
            }])
            .unwrap();
        let update = started.elapsed();
        let (analysis, _) = analyze(&mut session, &sources);
        if run > 0 {
            updates.push(update);
            analyses.push(analysis);
        }
    }
    println!(
        "{:<22} update {:>8.1} ms  analyze {:>9.1} ms",
        "tsconfig edit",
        median(updates),
        median(analyses)
    );
    // a file added and deleted
    let (mut updates, mut analyses) = (Vec::new(), Vec::new());
    let dir = target_path.rsplit_once('/').unwrap().0.to_owned();
    for run in 0..runs + 1 {
        version += 1;
        let path = format!("{dir}/__incbench_added.ts");
        let text = format!("export const added = {version};\n").into_bytes();
        let mut with = sources.clone();
        with.push(SourceFile {
            path: path.clone(),
            source: text.clone(),
            ..sources[target].clone()
        });
        let started = Instant::now();
        session
            .update([FileChange {
                path: path.clone(),
                source: text,
                deleted: false,
                version,
            }])
            .unwrap();
        let update = started.elapsed();
        let (analysis, _) = analyze(&mut session, &with);
        version += 1;
        session
            .update([FileChange {
                path,
                source: Vec::new(),
                deleted: true,
                version,
            }])
            .unwrap();
        analyze(&mut session, &sources);
        if run > 0 {
            updates.push(update);
            analyses.push(analysis);
        }
    }
    println!(
        "{:<22} update {:>8.1} ms  analyze {:>9.1} ms",
        "file added",
        median(updates),
        median(analyses)
    );
    let _ = session.close();
    let _ = BTreeSet::<u8>::new();
}
