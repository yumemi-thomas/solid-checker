# Exact own-data keys for owner guards

Regression fixture for ADR 0265: optional registration plus a mandatory
registration guarded by exactly target/onDown and a callable own onDown. The
assertions in expected-claims.json match the observed findings snapshot.

Module calls and the leaf's first call must be violations; the createRoot twin
and local homonym are clean. Extra/missing keys make the narrow guard false;
nonliteral handlers, spreads, accessors, computed keys, __proto__, proxies and
argument spreads leave it unknown. The original optional row keeps those calls
uncertifiable. No alias-order or mutation analysis is claimed.

Solid/signals/web stubs, manifest, JSX declarations and tsconfig are copied
byte-for-byte from package-derived-get-consumer. The synthetic signature is
fixture-owned; real pointer typings are checked separately. Authorization uses
the same manifest-bound closure and replaces only the exported symbol record.
No obsolete accepted-contracts catalog is shipped. Its hand-stated document is
counted in the live stableMainDocuments pin.
