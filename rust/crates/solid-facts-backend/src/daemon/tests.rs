use std::{
    cell::Cell,
    ffi::OsStr,
    fs,
    path::PathBuf,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    CheckHeader, CheckRequest, FileRefresh, ProcessMemory, Request, cache_retention_from,
    default_memory_limit_bytes, eligible, enabled_from, fingerprint_file, hash_file,
    inference_inputs_current, macho_build_identity, normalize_enablement, parse_meminfo_available,
    parse_process_memory, parse_vm_stat_available, process_tree_resident_bytes_from,
    refresh_file_with, retained_format, serve, socket_path, socket_path_in_domain, timing_value,
    verify_implementation,
};
use solid_reactive_ir::{CacheRetention, RuntimeEnvironment};

/// An ordinary check of an absolute project, daemon-eligible in every
/// respect but its effective dependency-source selection.
fn ordinary(selection: &[&str]) -> Request {
    serde_json::from_value(serde_json::json!({
        "projectId": "/project/tsconfig.json",
        "generation": 1,
        "sources": [],
        "typefactsExecutable": "/bin/typefacts",
        "dependencySources": selection,
    }))
    .unwrap()
}

#[test]
fn an_effective_dependency_selection_is_never_eligible() {
    // ADR 0311. The request alone decides: a named list, `*`, a
    // duplicate, a mix and a selection that selects nothing all bypass.
    assert!(eligible(&ordinary(&[])));
    for selection in [
        &["dep"][..],
        &["*"][..],
        &["dep", "dep"][..],
        &["dep", "*"][..],
        &["package-that-selects-nothing"][..],
        &[""][..],
    ] {
        assert!(!eligible(&ordinary(selection)), "{selection:?}");
    }
}

#[test]
fn serve_refuses_a_selection_before_it_reads_anything_else() {
    // The refusal is the first statement: this project, producer and
    // runtime do not exist, and no socket is bound.
    for selection in [&["dep"][..], &["*"][..], &[""][..]] {
        let error = serve(&ordinary(selection)).unwrap_err().to_string();
        assert!(error.contains("ADR 0311"), "{error}");
        assert!(error.contains("one-shot check"), "{error}");
    }
}

#[test]
fn release_default_and_explicit_daemon_policy_are_unambiguous() {
    assert!(enabled_from(None, true));
    assert!(!enabled_from(None, false));
    assert!(enabled_from(Some(OsStr::new("1")), false));
    assert!(enabled_from(Some(OsStr::new("true")), false));
    assert!(!enabled_from(Some(OsStr::new("0")), true));
    assert!(!enabled_from(Some(OsStr::new("false")), true));
    assert!(!enabled_from(Some(OsStr::new("unexpected")), true));
}

#[test]
fn large_projects_release_expensive_idle_indexes_by_default() {
    assert_eq!(cache_retention_from(None, 999), CacheRetention::Performance);
    assert_eq!(cache_retention_from(None, 1_000), CacheRetention::Balanced);
    assert_eq!(
        cache_retention_from(Some(OsStr::new("performance")), 5_000),
        CacheRetention::Performance
    );
    assert_eq!(
        cache_retention_from(Some(OsStr::new("compact")), 1),
        CacheRetention::Compact
    );
}

#[test]
fn retained_actor_supports_every_diagnostic_format() {
    assert!(retained_format("default"));
    assert!(retained_format("json"));
    assert!(retained_format("text"));
    assert!(!retained_format("sarif"));
}

#[test]
fn absent_daemon_enablement_fields_are_backward_compatible() {
    let mut absent: CheckRequest = serde_json::from_str(r#"{"projectId":"project"}"#).unwrap();
    normalize_enablement(&mut absent);
    assert!(absent.presets.is_empty());
    assert!(absent.enable_rules.is_empty());
}

#[test]
fn retained_inference_closure_observes_content_absence_membership_and_symlinks() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "daemon-inference-inputs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    fs::create_dir_all(&scratch).unwrap();
    let source = scratch.join("source.ts");
    fs::write(&source, "one").unwrap();
    let observed = |paths: &[PathBuf]| {
        paths
            .iter()
            .map(|path| {
                (
                    path.clone(),
                    solid_facts_backend::inferred_host_input_digest(path).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    let files = observed(std::slice::from_ref(&source));
    assert!(inference_inputs_current(&files));
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    fs::write(&source, "two").unwrap();
    fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(
        !inference_inputs_current(&files),
        "same size/mtime/inode cannot bless new bytes"
    );
    let absent = scratch.join("vite.config.js");
    let missing = observed(std::slice::from_ref(&absent));
    fs::write(&absent, "export default {};").unwrap();
    assert!(!inference_inputs_current(&missing));
    let directories = observed(std::slice::from_ref(&scratch));
    fs::write(scratch.join("excluded-server.ts"), "import './source';").unwrap();
    assert!(!inference_inputs_current(&directories));
    let alternate = scratch.join("alternate.ts");
    fs::write(&alternate, "two").unwrap();
    let link = scratch.join("linked.ts");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    let links = observed(std::slice::from_ref(&link));
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&alternate, &link).unwrap();
    assert!(
        !inference_inputs_current(&links),
        "equal target bytes do not imply equal link identity"
    );
    // A dangling link is a stable input: the answer is reused while it
    // dangles, and not once it is retargeted or its target appears.
    let dangling = scratch.join("optional-package");
    std::os::unix::fs::symlink(scratch.join("never-installed"), &dangling).unwrap();
    let dangles = observed(std::slice::from_ref(&dangling));
    assert!(inference_inputs_current(&dangles));
    fs::remove_file(&dangling).unwrap();
    std::os::unix::fs::symlink(scratch.join("also-missing"), &dangling).unwrap();
    assert!(
        !inference_inputs_current(&dangles),
        "a retargeted link moves"
    );
    let dangles = observed(std::slice::from_ref(&dangling));
    fs::create_dir_all(scratch.join("also-missing")).unwrap();
    assert!(
        !inference_inputs_current(&dangles),
        "an installed target moves"
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn daemon_enablement_order_and_duplicates_normalize_to_one_key() {
    let mut repeated = CheckRequest {
        implementation: String::from("test-checker"),
        project_id: "project".into(),
        presets: vec!["z".into(), "preferences".into(), "z".into()],
        enable_rules: vec![
            "prefer-show".into(),
            "prefer-for".into(),
            "prefer-show".into(),
        ],
        runtime: RuntimeEnvironment::default(),
        dependency_sources: vec!["dep-b".into(), "*".into(), "dep-b".into()],
        only_file: None,
    };
    normalize_enablement(&mut repeated);
    assert_eq!(repeated.presets, ["preferences", "z"]);
    assert_eq!(repeated.enable_rules, ["prefer-for", "prefer-show"]);
    assert_eq!(repeated.dependency_sources, ["*", "dep-b"]);
}

#[test]
fn a_dependency_source_domain_is_its_own_daemon() {
    // ADR 0296: the selection and host decide the analyzed program, so
    // another selection or host must not reach this daemon's answers.
    let base = socket_path("/p/tsconfig.json", "/bin/typefacts", "solid-v2", "c");
    assert_eq!(
        base,
        socket_path_in_domain("/p/tsconfig.json", "/bin/typefacts", "solid-v2", "c", None),
        "the ordinary daemon keeps its identity"
    );
    let domain = |text: &str| {
        socket_path_in_domain(
            "/p/tsconfig.json",
            "/bin/typefacts",
            "solid-v2",
            "c",
            Some(text),
        )
    };
    assert_ne!(base, domain("a"));
    assert_ne!(domain("a"), domain("b"));
    assert_eq!(domain("a"), domain("a"));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn the_running_executable_has_a_loaded_identity() {
    let identity = super::loaded_implementation_identity().unwrap();
    let digest = identity
        .strip_prefix(if cfg!(target_os = "macos") {
            "macho-uuid:"
        } else {
            "sha256:"
        })
        .unwrap();
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        digest.len(),
        if cfg!(target_os = "macos") { 32 } else { 64 }
    );
}

#[test]
fn loaded_macho_identity_requires_one_complete_build_uuid() {
    let mut command = Vec::new();
    command.extend(0x1b_u32.to_le_bytes());
    command.extend(24_u32.to_le_bytes());
    command.extend([7_u8; 16]);
    assert_eq!(
        macho_build_identity(&command, 1).as_deref(),
        Some("macho-uuid:07070707070707070707070707070707")
    );
    assert!(macho_build_identity(&command[..23], 1).is_none());
    assert!(macho_build_identity(&command, 0).is_none());
    assert!(macho_build_identity(&[], 0).is_none());
    let mut duplicate = command.clone();
    duplicate.extend(&command);
    assert!(macho_build_identity(&duplicate, 2).is_none());
    command[4..8].copy_from_slice(&0_u32.to_le_bytes());
    assert!(macho_build_identity(&command, 1).is_none());
}

#[test]
fn same_path_replacement_cannot_reuse_an_older_implementation() {
    let scratch =
        std::env::temp_dir().join(format!("daemon-checker-identity-{}", std::process::id()));
    fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("checker");
    fs::write(&path, "old checker bytes").unwrap();
    let old = format!("{:x?}", hash_file(&path).unwrap());
    let replacement = scratch.join("replacement");
    fs::write(&replacement, "new checker bytes").unwrap();
    fs::rename(&replacement, &path).unwrap();
    let new = format!("{:x?}", hash_file(&path).unwrap());
    assert_ne!(old, new);
    assert_ne!(
        socket_path("project", "typefacts", "solid-v2", &old),
        socket_path("project", "typefacts", "solid-v2", &new)
    );
    assert!(verify_implementation(&new, &old).is_err());
    assert!(
        verify_implementation(&new, "").is_err(),
        "pre-handshake daemon refused"
    );
    assert!(
        verify_implementation(&old, &new).is_err(),
        "server rejects a foreign client"
    );
    assert!(verify_implementation(&new, &new).is_ok());
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn retained_actor_identity_includes_project_and_typefacts_build() {
    let baseline = socket_path(
        "/project/a/tsconfig.json",
        "/bin/typefacts-a",
        "solid-v2",
        "checker-a",
    );
    assert_ne!(
        baseline,
        socket_path(
            "/project/b/tsconfig.json",
            "/bin/typefacts-a",
            "solid-v2",
            "checker-a"
        )
    );
    assert_ne!(
        baseline,
        socket_path(
            "/project/a/tsconfig.json",
            "/bin/typefacts-b",
            "solid-v2",
            "checker-a"
        )
    );
}

#[test]
fn process_tree_memory_includes_descendants_and_excludes_neighbors() {
    let parsed = parse_process_memory("10 1 100\n11 10 200\n12 11 300\n13 1 400\nmalformed\n");
    assert_eq!(
        parsed,
        vec![
            ProcessMemory {
                pid: 10,
                parent: 1,
                resident_kib: 100,
            },
            ProcessMemory {
                pid: 11,
                parent: 10,
                resident_kib: 200,
            },
            ProcessMemory {
                pid: 12,
                parent: 11,
                resident_kib: 300,
            },
            ProcessMemory {
                pid: 13,
                parent: 1,
                resident_kib: 400,
            },
        ]
    );
    assert_eq!(process_tree_resident_bytes_from(&parsed, 10), 600 * 1024);
}

#[test]
fn retained_timing_reports_cache_generation_and_payload() {
    let value = timing_value(
        &CheckHeader {
            implementation: String::from("test-checker"),
            ok: true,
            status: "certified".into(),
            error: String::new(),
            cache_hit: true,
            generation: 7,
            analysis_ns: 11,
            response_bytes: 13,
            notice: None,
            recovered: false,
        },
        Duration::from_nanos(17),
        13,
    );
    assert_eq!(value["mode"], "retained-daemon");
    assert_eq!(value["cacheHit"], true);
    assert_eq!(value["generation"], 7);
    assert_eq!(value["analysisNs"], 11);
    assert_eq!(value["roundtripNs"], 17);
    assert_eq!(value["responseBytes"], 13);
    assert_eq!(value["receivedBytes"], 13);
}

#[test]
fn unchanged_fingerprint_does_not_read_or_hash_source_content() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "solid-checker-daemon-fingerprint-{}-{nonce}.tsx",
        std::process::id()
    ));
    fs::write(&path, "export const value = 1;\n").expect("write fixture");
    let recorded = fingerprint_file(&path).expect("fingerprint fixture");
    let reads = Cell::new(0);

    let refresh = refresh_file_with(&path, &recorded, |path| {
        reads.set(reads.get() + 1);
        fs::read(path)
    })
    .expect("refresh fixture");

    assert!(matches!(refresh, FileRefresh::Unchanged));
    assert_eq!(reads.get(), 0, "unchanged source content was read");
    fs::remove_file(path).expect("remove fixture");
}

#[test]
fn metadata_change_with_same_content_reads_once_without_reporting_an_edit() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "solid-checker-daemon-metadata-{}-{nonce}.tsx",
        std::process::id()
    ));
    fs::write(&path, "export const value = 1;\n").expect("write fixture");
    let mut recorded = fingerprint_file(&path).expect("fingerprint fixture");
    recorded.stamp.changed_nanoseconds = recorded.stamp.changed_nanoseconds.wrapping_add(1);
    let reads = Cell::new(0);

    let refresh = refresh_file_with(&path, &recorded, |path| {
        reads.set(reads.get() + 1);
        fs::read(path)
    })
    .expect("refresh fixture");

    assert!(matches!(refresh, FileRefresh::MetadataOnly(_)));
    assert_eq!(reads.get(), 1);
    fs::remove_file(path).expect("remove fixture");
}

#[test]
fn changed_content_reads_once_and_reports_the_new_bytes() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "solid-checker-daemon-content-{}-{nonce}.tsx",
        std::process::id()
    ));
    fs::write(&path, "export const value = 1;\n").expect("write fixture");
    let recorded = fingerprint_file(&path).expect("fingerprint fixture");
    fs::write(&path, "export const value = 2;\n").expect("edit fixture");
    let reads = Cell::new(0);

    let refresh = refresh_file_with(&path, &recorded, |path| {
        reads.set(reads.get() + 1);
        fs::read(path)
    })
    .expect("refresh fixture");

    match refresh {
        FileRefresh::Content { bytes, .. } => {
            assert_eq!(bytes, b"export const value = 2;\n");
        }
        _ => panic!("changed content was not reported"),
    }
    assert_eq!(reads.get(), 1);
    fs::remove_file(path).expect("remove fixture");
}

#[test]
fn available_memory_parses_meminfo_and_vm_stat() {
    assert_eq!(
        parse_meminfo_available("MemTotal: 100 kB\nMemAvailable:   2048 kB\n"),
        Some(2048 * 1024)
    );
    assert_eq!(parse_meminfo_available("MemTotal: 100 kB\n"), None);
    let vm_stat = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
Pages free:                                   10.\n\
Pages active:                                 99.\n\
Pages inactive:                               20.\n\
Pages speculative:                             3.\n\
Pages purgeable:                               1.\n";
    // Purgeable pages are already among the inactive (or active) ones.
    assert_eq!(parse_vm_stat_available(vm_stat), Some(33 * 16384));
    assert_eq!(parse_vm_stat_available("garbage"), None);
}

#[test]
fn the_default_memory_limit_scales_with_the_machine() {
    let gib = 1024 * 1024 * 1024_u64;
    assert_eq!(default_memory_limit_bytes(None), 2 * gib);
    assert_eq!(default_memory_limit_bytes(Some(8 * gib)), 2 * gib);
    assert_eq!(
        default_memory_limit_bytes(Some(48 * gib)),
        48 * gib / 100 * 15
    );
    assert_eq!(default_memory_limit_bytes(Some(128 * gib)), 8 * gib);
}
