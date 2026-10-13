# Standard runtime configuration (ADRs 0266/0268)

App.tsx contains an untracked signal read and a tracked clean twin. The default
tsconfig excludes cases/. The process regression copies this tree and configures
exactly one case at a time, so no veto can contaminate a control.

Config, JSX and web stubs follow package-closed-map-owner-consumer. Solid and
signals use the first draft's full published rc.13 typings, unchanged (including
the seven exact name-node spans). They are not reduced signatures. The first
draft removed two unrelated upstream trailing spaces in store/utils.d.ts.
No hand-stated contract is added; live stableMainDocuments remains 204.

Vetoes: external call, renamed external call, DEV.hooks assignment, DEV alias,
destructure, Object.assign, descriptor, namespace member (including a TS wrapper),
OBSERVE.exclude and escape. Controls: baseline, truthiness-only DEV tests, local
homonyms, clean namespace, type-only import, value import used only in a typeof
type query, ordinary package export, intrinsics and dynamic import, external
capability read without a call. All retain the App read's violation.

The process assertions check kind, rule, primary span, dropped fixes, and the
other-file veto location. These expectations are authored, not observed. Run
fresh pinned-binary coverage comparison before generating the baseline snapshot.

Hydration cases use the real rc.13 `solid-js/internal` declaration of
sharedConfig (`types/internal.d.ts:138`, name bytes 7472..7484). Main `solid-js`
does not declare that export in these published typings; do not invent it in a
stub. Vetoes: load/has/gather writes, alias/re-export, Object.assign,
Object.defineProperty, escape/destructure, namespace plus TS wrapper, flag write,
delete, computed write, context escape, callback invocation and mixed benign/write
use. Controls: direct hydrating/done reads, truthiness-only object/context tests,
namespace reads, erased type-only/type-query uses and a same-named local.
All 22 new source programs have zero TypeScript diagnostics against published
rc.13 types (strict, noEmit, skipLibCheck=false). Process kinds are authored
expectations; compilation and fresh-binary coverage remain required at integration.
