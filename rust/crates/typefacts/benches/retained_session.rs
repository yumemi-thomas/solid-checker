//! Retained-session edit benchmark against the real Go producer.
//!
//! This measures what an editor pays per keystroke-scale edit on a large project:
//! one accepted update, the caller's own local work, and one analysis of ~1,000
//! demand groups of which exactly one changed.
//!
//! It compares three shapes. The first two differ *only* in where the caller's
//! work goes:
//!
//! - serial: acknowledge the update, then do local work, then analyse;
//! - pipelined: do local work while the update is in flight, then analyse.
//!
//! Both call the same session primitive, so the difference isolates overlap
//! rather than comparing two different call paths. Local work is a fixed number
//! of arithmetic rounds, not a sleep, so it consumes a real CPU and cannot
//! overlap by accident of the scheduler.
//!
//! The third differs from serial *only* in whether the caller still holds the
//! previous fact table when the next analysis lands:
//!
//! - serial_dropped_table: drop the table before every analysis.
//!
//! A held table co-owns the session's retained storage, so applying the next
//! delta must first copy every section it touches; a dropped one leaves the
//! session sole owner and the delta patches in place. The gap between serial
//! and serial_dropped_table is therefore the client-side cost of that copy —
//! paid by the ordinary editor pattern, which keeps the current table alive to
//! query until the next one replaces it.
//!
//! Output is JSON on stdout. There is deliberately no absolute time threshold:
//! the assertions here are correctness and relative overlap.
//!
//! Run with:
//!   cargo bench -p typefacts --bench retained_session

use std::{
    collections::BTreeMap,
    fs,
    hint::black_box,
    path::PathBuf,
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant},
};

use typefacts::{
    DemandGroup, FactTable, Location, Producer, Session,
    v3::{EntityDemand, FileChange},
};

const MODULES: usize = 1_000;
const DEMANDS_PER_MODULE: usize = 4;
const WARMUP_EDITS: usize = 3;
const MEASURED_EDITS: usize = 24;
/// Rounds of local work per edit. Sized so the work is comparable to a producer
/// update on a project this size, which is the regime where hiding it matters.
const LOCAL_WORK_ROUNDS: u64 = 12_000_000;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn producer() -> PathBuf {
    static PRODUCER: OnceLock<PathBuf> = OnceLock::new();
    PRODUCER
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("TYPEFACTS_TEST_BIN") {
                return PathBuf::from(path);
            }
            let output = repository_root()
                .join("target/typefacts-bench")
                .join(if cfg!(windows) {
                    "solid-typefacts.exe"
                } else {
                    "solid-typefacts"
                });
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            let ldflags = format!("-X main.buildID={}", typefacts::v3::TYPE_FACTS_BUILD_ID);
            let status = Command::new("go")
                .current_dir(repository_root())
                .args(["build", "-ldflags", &ldflags, "-o"])
                .arg(&output)
                .arg("./apps/solid-typefacts")
                .status()
                .expect("run go build for the retained-session benchmark");
            assert!(status.success(), "build the Type Facts benchmark producer");
            output
        })
        .clone()
}

/// One generated module: its path, the demands the client would hold for it, and
/// the two same-length source variants the edit script alternates between.
struct Module {
    path: String,
    demands: Vec<EntityDemand>,
    variants: [String; 2],
}

/// A deterministic TypeScript project plus the demand set a client would derive
/// from it.
///
/// Demand locations are recorded while the source is written, so the benchmark
/// never has to ask the producer where anything is — a real client holds its own
/// parser-derived locations too, and re-deriving them here would put work in the
/// timed region that no consumer actually pays.
struct Corpus {
    root: PathBuf,
    modules: Vec<Module>,
}

fn generate_corpus() -> Corpus {
    let root = repository_root().join("target/typefacts-bench-corpus");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create the benchmark corpus directory");

    fs::write(
        root.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "target": "ES2022"
  },
  "include": ["*.ts"]
}
"#,
    )
    .expect("write tsconfig");

    let mut shared = String::from("// Imported by every generated module.\n\n");
    for member in 0..DEMANDS_PER_MODULE {
        shared.push_str(&format!(
            "export function base{member}(value: number): number {{\n  return value + {member};\n}}\n\n"
        ));
    }
    fs::write(root.join("shared.ts"), &shared).expect("write shared.ts");

    // A mapped-type property access resolves to a synthesized symbol, which the
    // producer can only give a generation-scoped identity. One such file makes
    // the project non-durable, which is the ordinary case for real TypeScript and
    // used to force a whole-table pack on every edit.
    let mapped_path = root.join("mapped.ts");
    let mut mapped_source = String::from(
        "type Fields = { [K in \"first\" | \"second\"]: number };\n\ndeclare const fields: Fields;\n\n",
    );
    let mut mapped_demands = Vec::new();
    for member in ["first", "second"] {
        mapped_source.push_str(&format!("export const read_{member} = fields."));
        let start = mapped_source.len();
        mapped_source.push_str(member);
        let end = mapped_source.len();
        mapped_source.push_str(";\n");
        mapped_demands.push(EntityDemand {
            location: location(&mapped_path.to_string_lossy(), start, end),
            symbol: true,
            references: true,
            ..EntityDemand::default()
        });
    }
    fs::write(&mapped_path, &mapped_source).expect("write mapped.ts");

    let mut modules = Vec::with_capacity(MODULES + 1);
    modules.push(Module {
        path: mapped_path.to_string_lossy().into_owned(),
        demands: mapped_demands,
        // Never edited; it is here to make the project non-durable, not to change.
        variants: [mapped_source.clone(), mapped_source],
    });
    for index in 0..MODULES {
        let name = format!("mod{index:04}.ts");
        let path = root.join(&name);
        let (source, demands) = module_source(&path.to_string_lossy(), index, 0);
        // The alternate variant flips one digit, so it is byte-for-byte the same
        // length and every recorded demand location stays valid across edits.
        let (alternate, alternate_demands) = module_source(&path.to_string_lossy(), index, 1);
        assert_eq!(
            source.len(),
            alternate.len(),
            "edit variants must be the same length so demand spans do not shift"
        );
        assert_eq!(
            demands, alternate_demands,
            "edit variants must yield identical demand locations"
        );
        fs::write(&path, &source).expect("write generated module");
        modules.push(Module {
            path: path.to_string_lossy().into_owned(),
            demands,
            variants: [source, alternate],
        });
    }
    Corpus { root, modules }
}

/// Builds one module's source and the demand locations inside it, recording byte
/// offsets as the text is assembled.
fn module_source(path: &str, index: usize, variant: u8) -> (String, Vec<EntityDemand>) {
    let mut source = String::new();
    let mut demands = Vec::with_capacity(DEMANDS_PER_MODULE);

    source.push_str("import {");
    for member in 0..DEMANDS_PER_MODULE {
        if member > 0 {
            source.push(',');
        }
        source.push_str(&format!(" base{member}"));
    }
    source.push_str(" } from \"./shared\";\n\n");

    // The only difference between variants, and the reason spans hold: one digit.
    source.push_str(&format!("export const seed{index:04} = {variant};\n\n"));

    for member in 0..DEMANDS_PER_MODULE {
        source.push_str(&format!(
            "export function fn{index:04}_{member}(value: number): number {{\n  return base{member}(value) + seed{index:04};\n}}\n\n"
        ));

        source.push_str("export const ");
        let binding = format!("value{index:04}_{member}");
        let binding_start = source.len();
        source.push_str(&binding);
        let binding_end = source.len();
        source.push_str(" = ");

        let callee = format!("fn{index:04}_{member}");
        let callee_start = source.len();
        source.push_str(&callee);
        let callee_end = source.len();
        source.push_str(&format!("({member});\n\n"));

        // Two demands per member: the binding name, and the call it is
        // initialised by. Both shapes a real client asks for.
        demands.push(EntityDemand {
            location: location(path, binding_start, binding_end),
            symbol: true,
            references: member % 2 == 0,
            ..EntityDemand::default()
        });
        demands.push(EntityDemand {
            location: location(path, callee_start, callee_end),
            symbol: true,
            type_descriptor: true,
            resolved_call: true,
            ..EntityDemand::default()
        });
    }
    (source, demands)
}

fn location(path: &str, start: usize, end: usize) -> Location {
    Location {
        path: path.into(),
        start_byte: start as u64,
        end_byte: end as u64,
    }
}

/// A fixed amount of arithmetic standing in for the caller's own analysis. Not a
/// sleep: it must occupy a CPU, or "overlap" would prove nothing.
fn local_work() -> u64 {
    let mut accumulator = 0u64;
    for round in 0..LOCAL_WORK_ROUNDS {
        accumulator = accumulator
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(round | 1);
    }
    black_box(accumulator)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Serial,
    Pipelined,
}

/// Whether the caller keeps the previous fact table alive across the next
/// analysis. Holding is what a real editor does; dropping shows what the
/// analysis costs once the retained-table copy is off the path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Retention {
    Hold,
    Drop,
}

#[derive(Default)]
struct Samples {
    total: Vec<f64>,
    update_send: Vec<f64>,
    local_prep: Vec<f64>,
    update_wait: Vec<f64>,
    analyze_roundtrip: Vec<f64>,
    server_analyze: Vec<f64>,
    server_demand: Vec<f64>,
    server_assembly: Vec<f64>,
    request_bytes: Vec<f64>,
    response_bytes: Vec<f64>,
}

fn millis(value: Duration) -> f64 {
    value.as_secs_f64() * 1_000.0
}

fn percentile(values: &[f64], fraction: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let rank = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[rank]
}

fn summarize(values: &[f64]) -> BTreeMap<&'static str, f64> {
    let mut summary = BTreeMap::new();
    summary.insert("median", percentile(values, 0.50));
    summary.insert("p95", percentile(values, 0.95));
    summary
}

/// Runs the edit script in one shape and returns its samples plus the final
/// table, so the shapes can be proved equivalent.
fn run(shape: Shape, retention: Retention, corpus: &Corpus) -> (Samples, FactTable, u64) {
    let project_id = corpus
        .root
        .join("tsconfig.json")
        .to_string_lossy()
        .into_owned();
    let mut session =
        Session::open(Producer::at(producer()), &project_id, Vec::new()).expect("open session");

    let groups: Vec<DemandGroup<'_>> = corpus
        .modules
        .iter()
        .map(|module| DemandGroup::new(&module.demands).expect("non-empty demand group"))
        .collect();

    // Cold analysis: establishes retained state on both sides.
    session.analyze_groups(&groups).expect("first analysis");

    let mut samples = Samples::default();
    // The caller's copy of the current fact table. Under Hold it always tracks
    // the latest analysis, the way an editor keeps the current table alive to
    // query; under Drop it is released before every analysis, so the session
    // stays sole owner of the retained storage.
    let mut held: Option<FactTable> = None;

    for edit in 0..(WARMUP_EDITS + MEASURED_EDITS) {
        let measured = edit >= WARMUP_EDITS;
        // Index 0 is the never-edited mapped module.
        let target = 1 + edit % (corpus.modules.len() - 1);
        let module = &corpus.modules[target];
        let change = FileChange {
            path: module.path.clone(),
            version: edit as u64 + 1,
            source: module.variants[edit % 2].clone().into_bytes(),
            deleted: false,
        };

        // Exactly one of the ~1,000 groups differs from retained state: the
        // edited file's. Every other group is handed back unchanged.
        let mut edited = module.demands.clone();
        edited[0].references = !edited[0].references;
        let mut iteration_groups = groups.clone();
        iteration_groups[target] = DemandGroup::new(&edited).expect("edited group is non-empty");

        let started = Instant::now();
        let prep = match shape {
            Shape::Serial => {
                // Acknowledge first, then work.
                session
                    .update_during([change], || ())
                    .expect("serial update");
                let prep_started = Instant::now();
                black_box(local_work());
                prep_started.elapsed()
            }
            Shape::Pipelined => {
                // Work while the acknowledgement is in flight. The timer starts
                // inside the closure so local_prep means the same thing in both
                // shapes: the caller's own work, not the window containing it.
                session
                    .update_during([change], || {
                        let prep_started = Instant::now();
                        black_box(local_work());
                        prep_started.elapsed()
                    })
                    .expect("pipelined update")
            }
        };
        let update_timings = session.take_last_update_timings().unwrap_or_default();

        if retention == Retention::Drop {
            drop(held.take());
        }
        let analyze_started = Instant::now();
        let analyzed = session
            .analyze_groups(&iteration_groups)
            .expect("analysis after edit");
        let analyze_elapsed = analyze_started.elapsed();
        held = Some(analyzed);
        let total = started.elapsed();
        let exchange = session.take_last_exchange_timings().unwrap_or_default();

        if measured {
            samples.total.push(millis(total));
            samples.update_send.push(millis(update_timings.send));
            samples.local_prep.push(millis(prep));
            samples.update_wait.push(millis(update_timings.wait));
            samples.analyze_roundtrip.push(millis(analyze_elapsed));
            samples.server_analyze.push(millis(exchange.server_analyze));
            samples.server_demand.push(millis(exchange.server_demand));
            samples
                .server_assembly
                .push(millis(exchange.server_assembly));
            samples.request_bytes.push(exchange.request_bytes as f64);
            samples.response_bytes.push(exchange.response_bytes as f64);
        }

        // Restore the unedited demand run so the next iteration's "one changed
        // group" is measured against a stable baseline, under the same
        // retention discipline so every delta application in the run sees the
        // same ownership shape.
        if retention == Retention::Drop {
            drop(held.take());
        }
        let restored = session
            .analyze_groups(&groups)
            .expect("restore baseline demands");
        match retention {
            Retention::Hold => held = Some(restored),
            Retention::Drop => drop(restored),
        }
        black_box(&held);
    }

    // One final unchanged analysis, identical across shapes, so equivalence is
    // asserted on the same request regardless of shape or retention.
    drop(held);
    let final_table = session.analyze_groups(&groups).expect("final analysis");
    let generation = session.generation();
    session.close().expect("close session");
    (samples, final_table, generation)
}

fn emit(shape: &str, samples: &Samples, groups: usize, generation: u64) -> String {
    let field = |name: &str, values: &[f64]| {
        let summary = summarize(values);
        format!(
            "      \"{name}\": {{ \"median\": {:.4}, \"p95\": {:.4} }}",
            summary["median"], summary["p95"]
        )
    };
    let body = [
        field("total_edit_ms", &samples.total),
        field("update_send_ms", &samples.update_send),
        field("local_prep_ms", &samples.local_prep),
        field("update_wait_ms", &samples.update_wait),
        field("analyze_roundtrip_ms", &samples.analyze_roundtrip),
        field("server_analyze_ms", &samples.server_analyze),
        field("server_demand_ms", &samples.server_demand),
        field("server_assembly_ms", &samples.server_assembly),
        field("request_bytes", &samples.request_bytes),
        field("response_bytes", &samples.response_bytes),
    ]
    .join(",\n");
    format!(
        "    \"{shape}\": {{\n      \"samples\": {},\n      \"demand_groups\": {groups},\n      \"changed_groups_per_edit\": 1,\n      \"final_generation\": {generation},\n{}\n    }}",
        samples.total.len(),
        body
    )
}

fn main() {
    let corpus = generate_corpus();
    let groups = corpus.modules.len();

    let (serial, serial_table, serial_generation) = run(Shape::Serial, Retention::Hold, &corpus);
    let (pipelined, pipelined_table, pipelined_generation) =
        run(Shape::Pipelined, Retention::Hold, &corpus);
    let (dropped, dropped_table, dropped_generation) = run(Shape::Serial, Retention::Drop, &corpus);

    // Equivalence: every shape must land on identical facts, fact for fact,
    // or a faster number means nothing.
    assert_eq!(
        serial_table, pipelined_table,
        "serial and pipelined shapes produced different fact tables"
    );
    assert_eq!(
        serial_table, dropped_table,
        "held and dropped retention produced different fact tables"
    );
    assert_eq!(
        serial_generation, pipelined_generation,
        "serial and pipelined shapes ended on different session generations"
    );
    assert_eq!(
        serial_generation, dropped_generation,
        "held and dropped retention ended on different session generations"
    );
    let expected_generation = (WARMUP_EDITS + MEASURED_EDITS) as u64 + 1;
    assert_eq!(
        serial_generation, expected_generation,
        "every accepted update must advance exactly one generation"
    );

    println!("{{");
    println!("  \"benchmark\": \"retained_session\",");
    println!("  \"modules\": {},", MODULES);
    println!("  \"demands_per_module\": {},", DEMANDS_PER_MODULE * 2);
    println!("  \"measured_edits\": {},", MEASURED_EDITS);
    println!("  \"local_work_rounds\": {},", LOCAL_WORK_ROUNDS);
    println!("  \"shapes\": {{");
    println!("{},", emit("serial", &serial, groups, serial_generation));
    println!(
        "{},",
        emit("pipelined", &pipelined, groups, pipelined_generation)
    );
    println!(
        "{}",
        emit("serial_dropped_table", &dropped, groups, dropped_generation)
    );
    println!("  }}");
    println!("}}");
}
