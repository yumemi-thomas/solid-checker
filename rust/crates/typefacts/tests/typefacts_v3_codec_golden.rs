//! Cross-language codec conformance.
//!
//! Both fixtures are produced by Go (see
//! internal/typefacts/protocolv3_golden_test.go) and decoded here through the
//! deterministic-CBOR validator. Re-encoding must reproduce the file byte for
//! byte, so any drift in field names, canonical map ordering, or optional-field
//! omission between the two implementations fails this test.

use std::{fs, path::PathBuf};

use typefacts::{
    decode, encode,
    v3::{Operation, Request, Response, TYPE_FACTS_SCHEMA_V1},
};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../benchmarks/typefacts/phase1")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn request_golden_round_trips_identically() {
    let bytes = golden("typefacts-v3-request-golden.cbor");
    let request: Request = decode(&bytes).expect("decode Go request golden");

    assert_eq!(request.schema, TYPE_FACTS_SCHEMA_V1);
    assert_eq!(request.operation, Operation::Analyze);
    assert_eq!(request.request_id, 7);
    assert_eq!(request.state_token, "4");
    assert_eq!(request.changes.len(), 2);
    assert!(request.changes[1].deleted);
    let compact = request
        .compact_demands
        .as_ref()
        .expect("golden carries a compact demand snapshot");
    assert_eq!(compact.groups.len(), 2);
    assert_eq!(compact.strings[0], "");

    assert_eq!(encode(&request).expect("re-encode request"), bytes);
}

#[test]
fn response_golden_round_trips_identically() {
    let bytes = golden("typefacts-v3-response-golden.cbor");
    let response: Response = decode(&bytes).expect("decode Go response golden");

    assert_eq!(response.schema, TYPE_FACTS_SCHEMA_V1);
    assert!(response.ok);
    assert_eq!(response.state_token, "5");
    assert!(!response.table_transition.is_empty());

    assert_eq!(encode(&response).expect("re-encode response"), bytes);
}

/// The module-graph pair. Every optional field of every module row is populated
/// at least once across the two fixtures, so a drift in a field name, an enum
/// spelling, or an omission rule fails here rather than surviving until a
/// consumer reads a silently absent fact.
#[test]
fn module_graph_goldens_round_trip_identically() {
    use typefacts::{ModuleFormat, ModuleGraph, ModuleResolution};

    let request_bytes = golden("typefacts-module-graph-request-golden.cbor");
    let request: Request = decode(&request_bytes).expect("decode Go modules request golden");
    assert_eq!(request.operation, Operation::Modules);
    let demand = request
        .module_graph
        .as_ref()
        .expect("golden carries a module-graph demand");
    assert!(demand.imports && demand.packages);
    assert_eq!(demand.import_paths, vec!["/p/src/index.ts".to_string()]);
    assert_eq!(encode(&request).expect("re-encode request"), request_bytes);

    let response_bytes = golden("typefacts-module-graph-response-golden.cbor");
    let response: Response = decode(&response_bytes).expect("decode Go modules response golden");
    assert!(response.ok);
    assert!(response.table_transition.is_empty());
    assert_eq!(
        encode(&response).expect("re-encode response"),
        response_bytes
    );

    let graph = ModuleGraph {
        modules: response.modules,
        imports: response.module_imports,
        unknown_import_paths: response.unknown_import_paths,
    };

    // The inventory is path-ordered, so the typed lookup is a binary search.
    let entry = graph.module("/p/src/index.ts").expect("index.ts");
    assert_eq!(entry.format, ModuleFormat::Esm);
    assert!(!entry.declaration_file);
    assert!(entry.project_reference.is_none());

    let referenced = graph.module("/p/lib/src/channel.ts").expect("channel.ts");
    let mapping = referenced
        .project_reference
        .as_ref()
        .expect("a configured project reference covers this file");
    assert_eq!(&*mapping.source, "/p/lib/src/channel.ts");
    assert_eq!(&*mapping.output_dts, "/p/lib/dist/channel.d.ts");

    let vendored = graph
        .module("/p/node_modules/.store/reactive@4.2.0/node_modules/reactive/index.d.ts")
        .expect("the store copy");
    assert!(vendored.declaration_file);
    assert_eq!(vendored.format, ModuleFormat::Commonjs);
    assert_eq!(
        &*vendored.redirect_targets[0],
        "/p/vendor/reactive/index.d.ts"
    );
    assert_eq!(
        graph.module("/p/src/local-impl.ts").unwrap().format,
        ModuleFormat::Preserve
    );

    let imports: Vec<_> = graph.imports_from("/p/src/index.ts").collect();
    assert_eq!(imports.len(), 4);

    // A paths-aliased specifier: matched a `paths` key and did not land in
    // node_modules, so it is not the installed package of that name.
    let aliased = imports[0];
    assert_eq!(&*aliased.text, "reactive-package");
    assert_eq!(aliased.resolution, ModuleResolution::NonRelative);
    assert_eq!(&*aliased.paths_pattern, "reactive-package");
    assert!(aliased.package.is_none());

    // A pnpm-shaped install: both paths are reported, and the owning manifest
    // is the store copy's.
    let linked = imports[1];
    assert_eq!(linked.resolution, ModuleResolution::NodeModules);
    assert_eq!(&*linked.symlink_path, "/p/node_modules/reactive/index.d.ts");
    assert_ne!(&*linked.symlink_path, &*linked.resolved_path);
    let package = linked.package.as_ref().expect("owning package");
    assert_eq!((&*package.name, &*package.version), ("reactive", "4.2.0"));
    let resolver = linked.resolver_package.as_ref().expect("resolver identity");
    assert_eq!(&*resolver.name, "reactive");
    assert!(resolver.subpath.is_empty());

    // The one declaration-to-implementation join that exists.
    let redirected = imports[2];
    assert_eq!(&*redirected.extension, ".d.ts");
    assert_eq!(&*redirected.included_path, "/p/lib/src/channel.ts");
    assert!(redirected.ts_extension);

    let unresolved = imports[3];
    assert_eq!(unresolved.resolution, ModuleResolution::Unresolved);
    assert!(unresolved.resolved_path.is_empty());

    // A requested path the program did not hold is reported, so the answer
    // announces that it is scoped to less than what was asked for.
    assert!(!graph.is_complete());
    assert_eq!(&*graph.unknown_import_paths[0], "/p/src/absent.ts");
}
