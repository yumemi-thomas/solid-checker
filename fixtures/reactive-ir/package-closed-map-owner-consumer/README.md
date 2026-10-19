# Exact own-data keys for owner guards

ADR 0265: optional registration plus a mandatory registration guarded by exactly target/onDown and a callable own onDown: module calls and the leaf's first call are violations; the `createRoot` twin and local homonym clean; extra/missing keys make the guard false; nonliteral handlers, spreads, accessors, computed keys, `__proto__`, proxies and argument spreads leave it unknown. Stubs, manifest, JSX and tsconfig are byte copies of `package-derived-get-consumer`.
