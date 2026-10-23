//! ADR 0369: the producer's `calls_after_await` lists the calls on the
//! continuation and nothing else, so the consumers' guard
//! (`written_directly_in_async_flow`) never drops an entry. Held on the
//! fixtures whose async functions write calls in getters, setters,
//! constructors, class bodies and JSX after an await.
use std::{env, path::Path};

use solid_facts_backend::{NativeIncrementalSession, TypeFactsSession, dialect};

const FIXTURES: &[&str] = &[
    "reactive-ir/owner-after-await",
    "reactive-ir/read-after-await-reach/variants/nested-after-await",
    "reactive-ir/write-legality-proofs",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-getter-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-constructor-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-jsx-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-instance-field-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-static-block-after-await",
    "reactive-ir/dispatch-delivery-join/variants/dispatch-in-static-field-after-await",
];

#[test]
fn every_listed_call_after_await_is_on_the_continuation() {
    let Ok(typefacts) = env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    let mut listed = 0;
    for fixture in FIXTURES {
        let tsconfig = fixtures
            .join(fixture)
            .join("tsconfig.json")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let typescript = TypeFactsSession::open(&typefacts, &tsconfig, &[]).unwrap();
        let (mut session, _) = NativeIncrementalSession::open_pipelined(
            dialect::default_dialect(),
            tsconfig,
            typescript,
        )
        .unwrap();
        let facts = session.analyze().unwrap();
        listed += facts
            .typescript
            .files()
            .flat_map(|file| file.async_functions.iter())
            .map(|function| function.calls_after_await.len())
            .sum::<usize>();
        let off = solid_reactive_ir::calls_after_await_off_continuation(&facts);
        assert!(
            off.is_empty(),
            "{fixture}: listed off the continuation: {off:?}"
        );
    }
    // Not vacuous: the straight-line, static-block and static-field calls are listed.
    assert!(listed >= FIXTURES.len(), "only {listed} calls listed");
}
