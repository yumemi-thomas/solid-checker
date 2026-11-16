# stored-throw-render-run

**Claim (ADR 1006).** ADR 0893 makes a plain throw (`useContext` with no owner or no provider, a static `dynamic` source, `resolve`, `until`, `flush` in a tracked effect) a violation only where a run's throw reaches a report. A function the render runs in place, while it creates an element, throws where that element is created, so its throw ends where the element's does:

- a function an intrinsic `ref` applies, by an exact fact (`ref_application_of`): the attribute's literal (ADR 0893, unchanged), an element of an array literal, a branch of a conditional, or the literal a factory called in the attribute returns, the factory resolved exactly in the same file and never rebound;
- the function child of a dialect control-flow tag (`control_flow_parent`: `For` and `Repeat` at any arity, `Show` and `Match` with a parameter), which the tag runs under an owner of its own inside its compute; the render around the element reads that compute.

Violations: `RefArray`, `RefBranch`, `RefFactory`, `RefInRow`, `RefInRepeat`, `RefInShowChild` (SC4001, NO_OWNER_CONTEXT), `RowBody` (SC4002: a default-less context read in the row's body), `RowAsync` (an `async` row: the tag takes the promise and nothing handles its rejection).

Obligations (uncertifiable): `RefArrayHandled`, `RefInRowHandled`, `RowBodyHandled` (an `Errored` with a handling fallback above), `RefArrayAsyncCaught`, `RowAsyncCaught` (ADR 1008: the promise is caught), `RefMemberFactory` (a member call is no exact factory; Chrome does report it: a known miss), `RefRebound` (the name is reassigned), `ProjectChild` (a project component's function child: nothing says it is called). A control-flow tag that is not written in place in returned JSX keeps its row, because the tag's compute stores what the row throws and only a render that reads the tag reports it: `UnusedFor`, `UnusedRowBody`, `UnusedShow` (held in a binding nothing inserts: Chrome is silent), `DroppedFor` (a child of a project component that never reads its children: silent), `RowInExpression` (written in a child expression: not read here).

Silent: `RefArrayQuiet`, `RowReadsDefault` (the row runs under its own owner and the context has a default).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/stored-throw-render-run`: every violation fires its code at the site, every obligation but `RefMemberFactory`'s is silent. Premises: the closed program is the `private` manifest.
