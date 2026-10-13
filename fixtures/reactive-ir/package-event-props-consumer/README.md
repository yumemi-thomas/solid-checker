# package-event-props-consumer

**Claim (ADR 0207).** A package component whose accepted contract states an `event-handler-props` item runs every `on…` member of its props only at an external event, while its guard holds. A function literal written as such a prop then takes the role of an event handler written on an element (`ExecutionRole::EventCallback`). The proof follows ADR 0199 through project components (`execution_role::package_prop_runs_only_deferred`).

The contract of `reactive-package`'s `Root` states one callback item, `{ arg: 0, path: [], members: "event-handler-props" }`. Its `invoke` is at `external-event`, scheduled `external`, and guarded by `{ arg: 0, path: ["as"], kind: "plain" }`: `as` absent or a string, so no component receives the props. Every other callback is open.

| Case | Finding | Why |
| --- | --- | --- |
| `Direct`, `AnotherEvent` | none | an `on…` prop of `Root`, with no `as` and no spread |
| `Wrapped` | none | through `Wrap`, which spreads an `omit` view of its props; `Wrapped` writes no `as` and no spread |
| `AsComponent` | `SC1001` uncertifiable | `as` is a component, so the guard does not hold |
| `SpreadAtOrigin` | `SC1001` uncertifiable | the spread could carry `as` |
| `MergeWrapped` | `SC1001` uncertifiable | a merge can add `as` |
| `NotAnEvent` | `SC1001` uncertifiable | `render` is not an `on…` member |
| the import of `reactive-package` | `SC9005` uncertifiable | the contract leaves every other domain open, as it states nothing else |
| the import of `solid-js` | `SC9014` uncertifiable | the copied runtime stub names an unaudited release, as in `package-merged-props-consumer` |

The fixture is analyzed with the contract authorized out of band (`.solid-checker/authorize-contract.json`, as in `package-merged-props-consumer`). `package.json` is byte-identical to that fixture's, so the import block's digests hold; only `index.d.ts` and the contract differ. `solid-js.d.ts` is copied from `forwarded-event-prop` (`merge` and `omit` from `@solidjs/signals` rc.13).
