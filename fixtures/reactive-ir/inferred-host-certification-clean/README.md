# Inferred host: certification-clean

A clean inferred browser analysis cannot certify an explicit host premise (`--certify` refuses; an explicit browser overrides inference and may certify). `src/main.tsx` makes no package call, so no package behavior is involved, and the snapshot records status `uncertifiable` with no listed finding. Shape and Element/JSX signatures are those of `inferred-host-spa`; `host-cases.json` is checked against native discovery.
