# dependency-source-namespace-import

**Claim (ADR 0322).** A namespace import of selected package source
(`import * as Widget from "dep-ns/widget"`) whose every runtime reference is a
static member — a call `Widget.readNow(…)`, a read, or a JSX tag
`<Widget.Root>` — binds each member to the selected runtime export exactly as
the named import `import { readNow } from "dep-ns/widget"` would, and the
import loses its `NamespaceValue` obligation (`SC9005`). Type-position names
(`props: Widget.RootProps`, `typeof Widget.Root`) are erased and do not count.

| File | Case | Expected |
| --- | --- | --- |
| `App.tsx` | `StaticMembers`: `Widget.readNow(() => count())` in render | `SC1001` violation (the source reads the callback synchronously; same as the named-import control) |
| `App.tsx` | `Widget.readLater(() => count())` | clean (the source defers the read with `queueMicrotask`) |
| `App.tsx` | `Frame(props: Widget.RootProps)` | clean; the type name is erased |
| `Escaped.tsx` | `const all = Escaped` | the namespace escapes as a value: nothing is redirected and the call stays `SC1001` uncertifiable |
| `Computed.tsx` | `Computed[key](…)` | a computed member names nothing: `SC9012` and `SC1001` uncertifiable |

`dep-ns` is a two-file package written for this fixture (`exports: "./*"`,
peer `solid-js` 2.0.0-rc.13); its declarations match its runtime. The Solid
stubs are copied from `dependency-source-object-escape`. All sources pass
`tsc --noEmit` against these declarations. `.solid-checker/runtime.json`
selects `dep-ns/widget` as dependency source.
