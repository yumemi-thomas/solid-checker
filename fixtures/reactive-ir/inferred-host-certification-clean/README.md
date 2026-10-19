# Inferred host: certification-clean

A clean inferred browser analysis cannot certify an explicit host premise (`expected-claims.json`, hand-stated: `--certify` refuses, an explicit browser overrides inference and may certify). `src/main.tsx` makes no package call, so no package behavior is involved, and the snapshot records status `uncertifiable` with no listed finding. Shape and byte-faithful Element/JSX signatures come from `package-captured-lookup-consumer`; `host-cases.json` is checked against native discovery. The `solid-reactivity.json` beside the stub is inert.
