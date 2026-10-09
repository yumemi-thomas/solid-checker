# Standard runtime configuration (ADR 0266)

App.tsx contains an untracked signal read and a tracked clean twin. The default
tsconfig excludes cases/. The process regression copies this tree and configures
exactly one case at a time, so no veto can contaminate a control.

Config, JSX and web stubs follow package-closed-map-owner-consumer. Solid and
signals use the first draft's full published rc.13 typings, unchanged (including
the seven exact name-node spans). They are not reduced signatures. The first
draft removed two unrelated upstream trailing spaces in store/utils.d.ts.
No hand-stated contract is added; live stableMainDocuments remains 203.

Vetoes: external call, renamed external call, DEV.hooks assignment, DEV alias,
destructure, Object.assign, descriptor, namespace member (including a TS wrapper),
OBSERVE.exclude and escape. Controls: baseline, truthiness-only DEV tests, local
homonyms, clean namespace, type-only import, value import used only in a typeof
type query, ordinary package export, intrinsics and dynamic import, external
capability read without a call. All retain the App read's violation.

The process assertions check kind, rule, primary span, dropped fixes, and the
other-file veto location. These expectations are authored, not observed. Run
fresh pinned-binary coverage comparison before generating the baseline snapshot.
