# The `tsc` oracle

AGENTS.md's absolute rule: never report what TypeScript already reports, judged against the library's real published typings. Fixture stubs cannot answer that (a stub looser than the package invents defects: SC3004/SC9002 survived because stubs typed `apply` as `(value: T) => unknown` where `@solidjs/signals` says `EffectFunction = (v, p?) => (() => void) | void`). The oracle installs the packages in `packages.json` at the audited versions (`2.0.0-rc.17` triple of `solid-js`, `@solidjs/signals`, `@solidjs/web`; SRI integrities pinned there) and compiles against those. The checker half of every case must run with no SC9014 notice (`AUDITED_INSTALLATION`, `rust/crates/solid-dialect/src/solid_2/releases.rs`).

`rule-cases.json` holds one case per rule whose positive spelling could also be a `tsc` error, with `expect` (TypeScript) and `checker` (this checker) and a reason. `removed-because-redundant` requires `checker: "silent"`; `distinct-claim` requires `checker: "reports"` plus diagnostic codes and a reason naming what the finding claims that the type error does not; every exact dialect catalog rule needs a keystone (`expect: "silent"`, `checker: "reports"`) unless listed in the gate's `EXEMPT` map.

```sh
bun scripts/tsc-oracle.mjs provision --dialect all
bun scripts/tsc-oracle.mjs check --dialect v2 --code '<snippet>'
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/tsc-oracle-gate.mjs [--report]
```

The gate (`make tsc-oracle`) needs both binaries; an absent or version/integrity-drifted install is a hard failure, never a skip. Before adding or keeping a rule, write its positive case with `expect: "silent"` and `checker: "reports"`; if TypeScript speaks, the rule is TypeScript's. Move `packages.json` only with the checker's audited release.

## Redundancy ledger

Rules or arms removed because `tsc` reports them (executable record: `removed-because-redundant` cases; code record: `RETIRED_RULES` in `rust/crates/solid-facts-backend/src/dialect.rs`). Judged on rc.9 when each removal landed. The `removed-because-redundant` cases in `rule-cases.json` are re-checked against the pinned rc.17 by the gate; a row with no case there has not been re-judged on rc.17.

| Rule (code) | Removed claim | What `tsc` says | Still valid |
| --- | --- | --- | --- |
| `invalid-cleanup-return` (SC3004) | effect callback returns a non-cleanup | TS2345 / TS2322 `Type 'number' is not assignable to type 'void \| (() => void)'` | yes |
| `cleanup-return-unresolved` (SC9002) | uncertain legality of a returned value | TS2345 / TS2322 (mixed union, `unknown`, non-callback 2nd argument, unconstrained generic) | yes |
| `invalid-refresh-target`, `invalid-affects-target` (SC7003) | `refresh`/`affects` on a bad target | TS2345 (`Refreshable<T> = T & { readonly [$REFRESH]: any }`; `Accessor<unknown> \| Store<object>`) |yes; `{}` is TS2741 on 7.1 |
| `affects-keys-on-accessor` (SC7004) | key with an accessor target | TS2345 | yes |
| `refresh-target-unresolved`, `affects-target-unresolved` (SC9003) | uncertain brand | TS2345 unbranded, clean branded; TS2554 on arity | yes |
| `uncalled-accessor` (SC1005), typed positions and binary operators | accessor in a JSX attribute, class value, computed access, arithmetic/bitwise operand | TS2322, TS2538, TS2365, TS2362 | yes |
| `missing-effect-function` (SC7001), absent-apply arm | `createEffect(compute)` | TS2554 (rc.6 removed the one-argument overload) | yes on rc.9 |
| `missing-owner` (SC4004), return arm | `onSettled` callback returns a number | TS2322 against `onSettled(callback: () => void \| (() => void))` | yes |
| `no-implicit-draggable`; `reactive-dispatch-unresolved` (SC9012) invalid call; `server-function-rich-argument` (SC7007) arity arm | `draggable` shorthand; computed numeric call; extra rich argument | TS2322 `EnumeratedPseudoBoolean`; TS2349; TS2554 | yes |
| `no-store-mutation-outside-setter` (SC2003) root-record write | `state.count = 1` on a store root | TS2540 on rc.0-rc.6, clean on rc.9 (`Store<T> = T`): reversed, now a keystone | reversed |
| `no-store-mutation-outside-setter` (SC2003) accessor-binding arm | `count = 2` on a signal or memo accessor | TS2588 (`const`), TS2322 (`let` assigned a number, function or accessor-typed mismatch), TS2356 (`count++`), TS2365 (`count += 1`) on rc.17; a type-correct rebinding is silent in both and drops no write | yes (rc.17) |
| `no-store-mutation-outside-setter` (SC2003) accessor-member arm (`reactive-value`) | `memo.a = 1`, `count.x++`, `count["g"] = 1` on a signal or memo accessor | TS2339 (`Property 'a' does not exist on type 'SourceAccessor<number>'`), TS7053 for a computed key under `strict`; the cast spellings type-check and write a property on an ordinary function, which lands (rc.17: reads back, not frozen, no proxy), so no claim remains | yes (rc.17) |
| `no-store-mutation-outside-setter` (SC2003) `delete store.path` on a property TypeScript rejects | `delete store.req.id` on a required property; `delete store.v!`, `delete (store.v as T)`; `delete store.o.b` with `o?:` | TS2790 under strictNullChecks; TS2703; TS18048/TS18047; the delete arm (ADR 0460) reports only an optional, non-readonly property (Type Fact `writableOptionalProperty`), a keystone | yes (rc.17) |
| `jsx-no-duplicate-props` (SC8003), lowercase `innerhtml` arm | `innerhtml` counted as inner HTML beside children | TS2322 `Property 'innerhtml' does not exist ... Did you mean 'innerHTML'?` (rc.13) | yes |

Retired with Solid 1.x (ADR 0110; git tag `docs-archive-2026-10-19`): `v1/imports`, and the `v1/no-innerhtml`, `v1/jsx-no-duplicate-props`, `v1/event-handlers`, `v1/no-react-specific-props`, `v1/style-prop`, `v1/no-unknown-namespaces` arms `tsc` covers (TS2305, TS2322, TS17001, TS2783, TS1117, TS2710, TS2561, TS2353).

Kept because `tsc` is silent (keystones): SC1005 on string concatenation, unary `!`/`-`/`+`/`~` and template interpolation; SC2003 on nested store records, casts and props members (bare-identifier root check) and on `delete` of an optional store property; SC4002 (`onCleanup(() => {})` is a well-typed `Disposable`) and SC4004 on a real cleanup returned from an unowned `onSettled`; SC7001 on the cast-hidden non-callable apply and under `"use server"`. TypeScript skips the excess-property check for hyphenated JSX attributes but TS17001 still fires (`upstream_compat::jsx_name_is_type_checked`).

Real signatures the removals rest on (keep byte-faithful in stubs): `EffectFunction`, `onSettled(callback: () => void | (() => void))`, `createTrackedEffect(compute: () => void | (() => void), options?)`, `refresh<T>(target: Refreshable<T>)`, `affects(target: Accessor<unknown> | Store<object>)`, `affects<T extends object>(target: Store<T>, key: keyof T)`.
