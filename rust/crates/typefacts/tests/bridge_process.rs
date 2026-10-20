//! The bridged producer carrier against the process carrier, both over the
//! real producer. The bridge here drives a real child synchronously, one frame
//! at a time, which is exactly what an owned WASI reactor answers; the WASI
//! build itself is exercised by the WASM parity gate (`scripts/wasm-parity`).

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use typefacts::{
    AnalysisDemand, FactTable, FrameBridge, Location, ModuleGraphDemand, PrimitiveValueDomain,
    Producer, Session, SessionError,
    v3::{EntityDemand, FileChange, Request, Response},
};

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
            let output = repository_root().join("target/typefacts-test/solid-typefacts");
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            let ldflags = format!("-X main.buildID={}", typefacts::v3::TYPE_FACTS_BUILD_ID);
            let status = Command::new("go")
                .current_dir(repository_root())
                .args(["build", "-ldflags", &ldflags, "-o"])
                .arg(&output)
                .arg("./apps/solid-typefacts")
                .status()
                .expect("run go build for the bridge process test");
            assert!(status.success(), "go build failed");
            output
        })
        .clone()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    BadHandshake,
    Truncated,
    Trailing,
    Garbage,
    WrongRequest,
    WrongGeneration,
    WrongProject,
    Service,
}

/// A real producer child driven one synchronous frame at a time, with one
/// optional fault injected into the `fault_at`-th exchange's answer.
struct ChildBridge {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    handshake: Vec<u8>,
    closes: Arc<AtomicUsize>,
    exchanges: Arc<AtomicUsize>,
    fault: Fault,
    fault_at: usize,
    recording: Option<Recording>,
}

/// Every frame one bridge carried, written for the WASM parity gate to replay
/// into the packaged producer reactor over the same tree
/// (`scripts/wasm-parity.mjs --update-frames`).
struct Recording {
    directory: PathBuf,
    project: PathBuf,
    exchanges: Vec<String>,
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Recording {
    /// Set by `TYPEFACTS_BRIDGE_FRAMES`; absent, nothing is written.
    fn from_environment(project: &Path, handshake: &[u8]) -> Option<Self> {
        let directory = PathBuf::from(std::env::var_os("TYPEFACTS_BRIDGE_FRAMES")?);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("handshake.bin"), handshake).unwrap();
        Some(Self {
            directory,
            project: project.to_path_buf(),
            exchanges: Vec::new(),
        })
    }

    fn exchange(&mut self, request: &[u8], response: &[u8]) {
        let index = self.exchanges.len();
        let operation = typefacts::decode_trusted::<Request>(&request[4..])
            .map(|request| format!("{:?}", request.operation).to_lowercase())
            .unwrap_or_else(|_| "undecodable".into());
        let request_file = format!("{index:03}-{operation}-request.bin");
        let response_file = format!("{index:03}-{operation}-response.bin");
        fs::write(self.directory.join(&request_file), request).unwrap();
        fs::write(self.directory.join(&response_file), response).unwrap();
        self.exchanges.push(format!(
            "{{\"operation\":{},\"request\":{},\"response\":{}}}",
            json_string(&operation),
            json_string(&request_file),
            json_string(&response_file)
        ));
    }

    fn finish(&self) {
        let project = self.project.to_string_lossy();
        let root = self.project.parent().unwrap().to_string_lossy();
        fs::write(
            self.directory.join("frames.json"),
            format!(
                "{{\"root\":{},\"projectId\":{},\"handshake\":\"handshake.bin\",\"exchanges\":[{}]}}\n",
                json_string(&root),
                json_string(&project),
                self.exchanges.join(",")
            ),
        )
        .unwrap();
    }
}

fn read_raw_frame(reader: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut prefix = [0_u8; 4];
    reader.read_exact(&mut prefix)?;
    let mut payload = vec![0_u8; u32::from_le_bytes(prefix) as usize];
    reader.read_exact(&mut payload)?;
    let mut frame = prefix.to_vec();
    frame.extend(payload);
    Ok(frame)
}

impl ChildBridge {
    fn spawn(
        project: &Path,
        fault: Fault,
        fault_at: usize,
        record: bool,
    ) -> (Self, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let mut child = Command::new(producer())
            .args(["-schema=1", "-inline-sources", "-project"])
            .arg(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut handshake = read_raw_frame(&mut stdout).unwrap();
        if fault == Fault::BadHandshake {
            *handshake.last_mut().unwrap() ^= 0x01;
        }
        let closes = Arc::new(AtomicUsize::new(0));
        let exchanges = Arc::new(AtomicUsize::new(0));
        let recording = if record {
            Recording::from_environment(project, &handshake)
        } else {
            None
        };
        (
            Self {
                child,
                stdin,
                stdout,
                handshake,
                closes: Arc::clone(&closes),
                exchanges: Arc::clone(&exchanges),
                fault,
                fault_at,
                recording,
            },
            closes,
            exchanges,
        )
    }

    fn corrupt(&self, frame: Vec<u8>) -> Vec<u8> {
        let rewrite = |change: &dyn Fn(&mut Response)| {
            let mut response: Response = typefacts::decode_trusted(&frame[4..]).unwrap();
            change(&mut response);
            let payload = typefacts::encode(&response).unwrap();
            let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
            frame.extend(payload);
            frame
        };
        match self.fault {
            Fault::None | Fault::BadHandshake => frame,
            Fault::Truncated => frame[..frame.len() - 1].to_vec(),
            Fault::Trailing => {
                let mut frame = frame;
                frame.push(0);
                frame
            }
            Fault::Garbage => {
                let mut frame = 8_u32.to_le_bytes().to_vec();
                frame.extend([0xff; 8]);
                frame
            }
            Fault::WrongRequest => rewrite(&|response| response.request_id += 1),
            Fault::WrongGeneration => rewrite(&|response| response.generation += 1),
            Fault::WrongProject => rewrite(&|response| response.project_id.push('x')),
            Fault::Service => rewrite(&|response| {
                response.ok = false;
                response.error = Some(typefacts::v3::Error {
                    code: "analysis-failed".into(),
                    message: "injected".into(),
                });
            }),
        }
    }
}

impl FrameBridge for ChildBridge {
    fn handshake(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.handshake.clone())
    }

    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, String> {
        let index = self.exchanges.fetch_add(1, Ordering::SeqCst) + 1;
        self.stdin
            .write_all(request)
            .map_err(|error| error.to_string())?;
        self.stdin.flush().map_err(|error| error.to_string())?;
        let frame = read_raw_frame(&mut self.stdout).map_err(|error| error.to_string())?;
        if let Some(recording) = &mut self.recording {
            recording.exchange(request, &frame);
        }
        Ok(if index == self.fault_at {
            self.corrupt(frame)
        } else {
            frame
        })
    }

    fn close(&mut self) {
        self.closes.fetch_add(1, Ordering::SeqCst);
        if let Some(recording) = &self.recording {
            recording.finish();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Fixture {
    project: PathBuf,
    source: PathBuf,
    source_text: String,
    origin: PathBuf,
    script: PathBuf,
}

fn fixture(name: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!("typefacts-bridge-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("node_modules/lib")).unwrap();
    let root = root.canonicalize().unwrap();
    let source_text = concat!(
        "import { Widget, helper } from \"./origin\";\n",
        "import { value } from \"lib\";\n",
        "declare const literal: \"a\" | \"b\";\n",
        "let assigned = () => 0;\n",
        "export { Widget };\n",
        "export const derived = helper(value);\n",
        "export const exported = (n: number) => helper(n);\n",
        "const probe = Widget;\n",
        "const usesLiteral = literal;\n",
        "const runtime = assigned;\n",
        "export { probe, usesLiteral, runtime };\n",
        "function local(n: number): number { return n; }\n",
        "export const called = local(2);\n",
    );
    for (path, contents) in [
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noEmit":true,"target":"esnext","module":"esnext","moduleResolution":"bundler","allowJs":true,"types":[]},"include":["*.ts","*.js"]}"#,
        ),
        (
            "origin.ts",
            "export class Widget {}\nexport const helper = (value: number) => value + 1;\n",
        ),
        ("source.ts", source_text),
        (
            "script.js",
            "const lib = require(\"lib\");\nmodule.exports = lib;\n",
        ),
        ("empty.ts", ""),
        (
            "node_modules/lib/package.json",
            r#"{"name":"lib","version":"2.0.0","main":"index.js","types":"index.d.ts"}"#,
        ),
        (
            "node_modules/lib/index.d.ts",
            "export declare const value: number;\n",
        ),
        ("node_modules/lib/index.js", "exports.value = 1;\n"),
    ] {
        fs::write(root.join(path), contents).unwrap();
    }
    Fixture {
        project: root.join("tsconfig.json"),
        source: root.join("source.ts"),
        source_text: source_text.into(),
        origin: root.join("origin.ts"),
        script: root.join("script.js"),
    }
}

fn location(fixture: &Fixture, needle: &str, name: &str) -> Location {
    let start = fixture.source_text.find(needle).unwrap() + needle.find(name).unwrap();
    Location {
        path: fixture.source.to_string_lossy().into_owned().into(),
        start_byte: start as u64,
        end_byte: (start + name.len()) as u64,
    }
}

fn demands(fixture: &Fixture) -> AnalysisDemand {
    let mut entities = vec![
        EntityDemand {
            location: location(fixture, "const probe = Widget;", "Widget"),
            symbol: true,
            references: true,
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: location(fixture, "const usesLiteral = literal;", "literal"),
            symbol: true,
            primitive_value_domain: true,
            primitive_literal_candidates: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: location(fixture, "const runtime = assigned;", "assigned"),
            symbol: true,
            // The binding kind is produced only beside both type answers.
            runtime_identity: true,
            callability: true,
            constructability: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: location(fixture, "helper(value)", "helper(value)"),
            resolved_call: true,
            call_result_domain: true,
            ..EntityDemand::default()
        },
    ];
    // One run per path, in source order: the canonical demand shape.
    entities.sort_by_key(|demand| (demand.location.start_byte, demand.location.end_byte));
    AnalysisDemand { entities }
}

fn rows(table: &FactTable) -> (Vec<String>, Vec<String>, Vec<String>) {
    (
        table
            .entities()
            .map(|entity| format!("{entity:?}"))
            .collect(),
        table
            .symbols()
            .map(|symbol| format!("{symbol:?}"))
            .collect(),
        table.files().map(|file| format!("{file:?}")).collect(),
    )
}

fn open_pair(fixture: &Fixture, record: bool) -> (Session, Session, Arc<AtomicUsize>) {
    let process = Session::open(
        Producer::at(producer()),
        fixture.project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let (bridge, closes, _) = ChildBridge::spawn(&fixture.project, Fault::None, 0, record);
    let bridged =
        Session::open_bridged(Box::new(bridge), fixture.project.to_string_lossy()).unwrap();
    (process, bridged, closes)
}

#[test]
fn bridged_answers_equal_process_answers() {
    let fixture = fixture("equal");
    let (mut process, mut bridged, closes) = open_pair(&fixture, true);
    assert!(bridged.cancellation_handle().is_none());
    assert!(process.cancellation_handle().is_some());

    let sources = process.configured_sources().unwrap();
    assert_eq!(bridged.configured_sources().unwrap(), sources);
    assert!(sources.iter().all(|source| !source.local));
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("empty.ts") && source.source.is_empty())
    );
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("script.js") && !source.source.is_empty())
    );

    let missing = fixture.source.with_file_name("missing.ts");
    let graph_demand = ModuleGraphDemand::default()
        .import_paths([
            fixture.source.to_string_lossy().into_owned(),
            fixture.script.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
        ])
        .with_packages();
    let graph = process.module_graph(&graph_demand).unwrap();
    assert_eq!(bridged.module_graph(&graph_demand).unwrap(), graph);
    assert_eq!(graph.unknown_import_paths.len(), 1);
    assert!(
        graph
            .imports
            .iter()
            .any(|row| row.specifier.path.ends_with("script.js") && row.text.as_ref() == "lib")
    );

    let demand = demands(&fixture);
    let expected = process.analyze(&demand).unwrap();
    let actual = bridged.analyze(&demand).unwrap();
    assert_eq!(rows(&actual), rows(&expected));
    // Non-default witnesses of the fields the JSON prototype could not carry,
    // and a symbol closure wider than the four demanded entities.
    let entities = expected.entities().collect::<Vec<_>>();
    assert!(
        entities
            .iter()
            .any(|entity| entity.constructability.is_some())
    );
    assert!(
        entities
            .iter()
            .any(|entity| entity.primitive_value_domain != PrimitiveValueDomain::default())
    );
    assert!(
        entities
            .iter()
            .any(|entity| entity.primitive_literal_candidates.is_some())
    );
    assert!(
        entities
            .iter()
            .any(|entity| entity.runtime_binding_kind.is_some())
    );
    assert!(
        expected.symbols().len() > 3,
        "{} symbols",
        expected.symbols().len()
    );

    // Reuse, then an update and a delta.
    assert_eq!(
        rows(&bridged.analyze(&demand).unwrap()),
        rows(&process.analyze(&demand).unwrap())
    );

    let change = FileChange {
        path: fixture.origin.to_string_lossy().into_owned(),
        source: b"export class Widget {}\nexport const helper = (value: number) => value * 2;\nexport const extra = 1;\n".to_vec(),
        deleted: false,
        version: 1,
    };
    process.update([change.clone()]).unwrap();
    bridged.update([change]).unwrap();
    assert_eq!(bridged.generation(), 2);
    let expected = process.analyze(&demand).unwrap();
    let actual = bridged.analyze(&demand).unwrap();
    assert_eq!(rows(&actual), rows(&expected));
    assert_eq!(
        bridged.take_last_table_changes(),
        process.take_last_table_changes()
    );
    let sources = process.configured_sources().unwrap();
    assert_eq!(bridged.configured_sources().unwrap(), sources);
    assert!(sources.iter().any(
        |source| source.path.ends_with("origin.ts") && source.source.ends_with(b"extra = 1;\n")
    ));

    bridged.close().unwrap();
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    process.close().unwrap();
}

/// Every declared name in `source`: the span after each `function`, `const`,
/// `let` or `class` keyword, as one symbol-and-references demand.
fn declared_names(path: &str, source: &[u8]) -> Vec<EntityDemand> {
    let text = String::from_utf8_lossy(source);
    let mut demands = Vec::new();
    for keyword in ["function ", "const ", "let ", "class "] {
        for (at, _) in text.match_indices(keyword) {
            let start = at + keyword.len();
            let length = text[start..]
                .find(|character: char| !(character.is_alphanumeric() || character == '_'))
                .unwrap_or(text.len() - start);
            if length == 0 {
                continue;
            }
            demands.push(EntityDemand {
                location: Location {
                    path: path.into(),
                    start_byte: start as u64,
                    end_byte: (start + length) as u64,
                },
                symbol: true,
                references: true,
                callability: true,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

/// The checked-in forms and dependency-source projects through both carriers:
/// sources, the complete module graph with packages, and every declared name.
#[test]
fn bridged_answers_equal_process_answers_over_fixtures() {
    for relative in [
        "apps/solid-typefacts/prototypes/typefacts-reactor/testdata/forms/tsconfig.json",
        "fixtures/reactive-ir/dependency-source-program/tsconfig.json",
    ] {
        let project = repository_root().join(relative).canonicalize().unwrap();
        let mut process = Session::open(
            Producer::at(producer()),
            project.to_string_lossy(),
            Vec::new(),
        )
        .unwrap();
        let (bridge, closes, _) = ChildBridge::spawn(&project, Fault::None, 0, false);
        let mut bridged =
            Session::open_bridged(Box::new(bridge), project.to_string_lossy()).unwrap();

        let sources = process.configured_sources().unwrap();
        assert_eq!(bridged.configured_sources().unwrap(), sources, "{relative}");
        let graph_demand = ModuleGraphDemand::default()
            .import_paths(sources.iter().map(|source| source.path.clone()))
            .with_packages();
        let graph = process.module_graph(&graph_demand).unwrap();
        assert_eq!(
            bridged.module_graph(&graph_demand).unwrap(),
            graph,
            "{relative}"
        );
        assert!(!graph.imports.is_empty(), "{relative}");
        assert!(
            graph.imports.iter().any(|row| row.package.is_some()),
            "{relative}"
        );

        let mut entities = sources
            .iter()
            .filter(|source| !source.path.ends_with(".d.ts"))
            .flat_map(|source| declared_names(&source.path, &source.source))
            .collect::<Vec<_>>();
        entities.sort_by(|left, right| {
            (&left.location.path, left.location.start_byte)
                .cmp(&(&right.location.path, right.location.start_byte))
        });
        let demand = AnalysisDemand { entities };
        let expected = process.analyze(&demand).unwrap();
        assert_eq!(
            rows(&bridged.analyze(&demand).unwrap()),
            rows(&expected),
            "{relative}"
        );
        assert!(expected.entities().count() > 3, "{relative}");
        assert!(
            expected.entities().any(|entity| !entity.symbol.is_empty()),
            "{relative}"
        );

        bridged.close().unwrap();
        assert_eq!(closes.load(Ordering::SeqCst), 1, "{relative}");
        process.close().unwrap();
    }
}

#[test]
fn a_bad_handshake_refuses_the_bridge_and_releases_it() {
    let fixture = fixture("handshake");
    let (bridge, closes, exchanges) =
        ChildBridge::spawn(&fixture.project, Fault::BadHandshake, 0, false);
    assert!(matches!(
        Session::open_bridged(Box::new(bridge), fixture.project.to_string_lossy()),
        Err(SessionError::Handshake(_))
    ));
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert_eq!(exchanges.load(Ordering::SeqCst), 0);
}

#[test]
fn any_bridged_failure_closes_the_session_without_replay() {
    for fault in [
        Fault::Truncated,
        Fault::Trailing,
        Fault::Garbage,
        Fault::WrongRequest,
        Fault::WrongGeneration,
        Fault::WrongProject,
        Fault::Service,
    ] {
        let fixture = fixture("fault");
        // Exchange 1 opens; exchange 2 is the analysis that fails.
        let (bridge, closes, exchanges) = ChildBridge::spawn(&fixture.project, fault, 2, false);
        let mut bridged =
            Session::open_bridged(Box::new(bridge), fixture.project.to_string_lossy()).unwrap();
        assert!(bridged.analyze(&demands(&fixture)).is_err());
        assert_eq!(
            closes.load(Ordering::SeqCst),
            1,
            "the failed bridge was not released"
        );
        let after = exchanges.load(Ordering::SeqCst);
        assert_eq!(after, 2, "a failed bridge was retried or replayed");
        assert!(matches!(
            bridged.configured_sources(),
            Err(SessionError::Closed)
        ));
        assert!(matches!(
            bridged.analyze(&demands(&fixture)),
            Err(SessionError::Closed)
        ));
        assert_eq!(
            exchanges.load(Ordering::SeqCst),
            after,
            "a closed bridge was reused"
        );
    }
}
