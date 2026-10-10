# rc9-reexport-gap-import-equals

A TypeScript import-equals, `import S = require("solid-js")`, whose every use
is a member read. `S` is the module's namespace object exactly as
`import * as S` binds it, so the census reads it the same way: `S.untrack`
names an export rc.9 declares and reaches nothing; `S.createErrorBoundary`
names one of the five it does not (`rc9-reexport-gap-none` explains the gap
and the stubs, which are the same here). An `S` passed or stored would keep
the gap open, as it does for a namespace import.

No import declaration and no call records this form, so the syntax facts
carry it separately (`AstFacts::import_equals`, facts schema 46).

Expected: the `SC9014` notice carries the re-export gap, naming only
`createErrorBoundary`, located at the member read. Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).

`tsconfig.json` uses `"module": "Preserve"`, under which TypeScript accepts an
import-equals of a package. `tsc --noEmit` (5.9.3, `strict`, bundler
resolution, `skipLibCheck`) is clean against these stubs and against the
published `solid-js`, `@solidjs/signals` and `@solidjs/web` `2.0.0-rc.9`.
