# dead-literal-rows

**Claim (ADR 1015).** Children a LITERAL prop keeps from ever running are unreached code, so a violation written there is uncertifiable (`unreached-dead-code`, ADR 0458), never a violation: Chrome raises nothing there (`fixtures/runtime-oracle/dead-literal-rows`, quiet; its pair `dead-literal-rows-live` raises `CONTEXT_NOT_FOUND` at every live control). The Solid 2 dialect says which prop and which literal (`Dialect::flow_children_exclusion`, read on the rc.21 triple only): `<For each={[]}>` (its row function's body: `For` reads `props.children` once when it maps), `<Repeat count={0}>`, `<Show when={false}>` and `<Match when={false}>` (their children are never read). The tag must resolve EXACTLY to the Solid-owned declaration (every declaration in the dialect's packages under one export name), the prop must be written once with no namespace and hold exactly the literal (under parentheses and a TypeScript assertion), and the element must carry no spread.

`SC4002` of the same read (`useContext(Bare)` with no provider), by function:

| function | outcome | why |
|---|---|---|
| `RepeatZeroTracked`, `ForEmptyTracked`, `ShowFalseTracked`, `MatchFalseTracked`, `RepeatZeroElementChild` | uncertifiable | the literal rows and branches (`Repeat` also for a child that is no function: its mapper reads `props.children`) |
| `RepeatZeroAlias`, `ForEmptyNamespace` | uncertifiable | the tag is the declaration, not the spelling: `import { Repeat as Rep }`, `<S.For>` |
| `RepeatZeroParenthesized`, `ShowFalseAsserted` | uncertifiable | `count={(0)}`, `when={false as boolean}` |
| `ForNestedInDeadRepeat` | uncertifiable | a live `For` written in a dead `Repeat` row |
| `RepeatOneTracked`, `ForOneTracked`, `ShowTrueTracked` | violation | the same read where the children may run |
| `ForEmptyFallbackTracked`, `RepeatZeroFallbackTracked`, `ShowFalseFallbackTracked` | violation | a `fallback` runs exactly when the children do not |
| `ForEmptyRowMadeByCall` | violation | `For` reads `props.children` once: `makeRow()` is evaluated, only the row function's body is dead |
| `RepeatNamedZero`, `ForNamedEmpty`, `ShowNegated` | violation | not the literal: a name, a negation. Anything but the literal keeps "may run", whatever the value is at run time (the owner's ruling, option c). **A known remaining false positive**: Chrome is silent where `zero` is 0 |
| `RepeatZeroWithSpread` | violation | a spread may replace the prop |
| `Shadowed.tsx`: a project `Repeat` and `Show` | their rows (U and violation) | a project component of that name is no flow component: it calls its children |

The stub (`node_modules/solid-js/index.d.ts`) copies `For`, `Repeat`, `Show`, `Switch`, `Match`, `Context`, `createContext` and `useContext` from the published `solid-js@2.0.0-rc.21` typings; the program is closed (`.solid-checker/runtime.json`).
