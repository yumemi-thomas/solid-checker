# Inferred host: certification-clean

A clean inferred browser analysis cannot certify an explicit host premise (`--certify` refuses; an explicit browser overrides inference and may certify). `src/main.tsx` makes no package call, so no package behavior is involved; the snapshot records status `uncertifiable` with no listed finding, which is the admitted host's mark. `host-cases.json` is checked against native discovery by `native_execution_fixture_sites_keep_function_boundaries_and_edge_refusals` (`inferred_host/tests.rs`). Shape and Element/JSX signatures are those of `inferred-host-spa`.
