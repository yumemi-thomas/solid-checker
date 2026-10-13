# ADR 0166: A host constant is folded per certified host

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners: the host constant resolver
  (`rust/crates/solid-facts-backend/src/host_constants.rs`), its syntax half
  (`rust/crates/solid-facts/src/ast/host_constants.rs`, and the `folds` argument
  of `ast/unconditional_calls.rs`), the certification's private project
  (`contract_certification/type_facts.rs`, `private_project_host_constants`),
  the emission fold (`main.rs`, `fold_emission_host_constants`), the Type Facts
  producer's request field and decision (`protocolv3.go` `HostConstant`,
  `tsgo/host_constants.go`, `tsgo/unconditional_calls.go`), and the wire
  (`typefacts/src/v3.rs`, `schema/typefacts-v1.schema.json`, handshake
  protocol 75)
- Relation: builds on ADR 0140 (a package is certified per host) and ADR 0159
  (a strict reachability floor is a lower bound). Closes the host-blindness ADR
  0140 left open ("a package whose closure was host-neutral"): a certification
  now reads the host it declares.

## Context

ADR 0140 certifies a package once per host, and a case carries the host
condition, so a `browser` case reaches only a consumer that declared `browser`.
Nothing inside a certification used the host, though. Every @solid-primitives
package that guards server work does so at run time:

```js
import { isServer } from "@solidjs/web";
function onElementConnect(el, fn) {
	if (isServer) return;
	if (el.isConnected) return fn();
	...
	onCleanup(() => observer.disconnect());
}
```

`@solidjs/web@2.0.0-rc.9` publishes `const isServer = false` in every file its
`exports` map selects for `browser` (`dist/web.js`, `web.dev.js`,
`web.observe.js`) and `const isServer = true` in every file it selects for
`node` (`dist/server.js`, `server.dev.js`, `server.observe.js`). The published
declaration is `export declare const isServer: boolean` on every host, so no
declaration decides it (and `tsc` reports nothing about these guards). Three
consequences, measured on the pinned `@solid-primitives` corpus:

- `onElementConnect`'s proposed `returns` (a plain return through `fn()`)
  contradicts under `node`: the guard always returns, so the runtime veto sees
  `undefined`. The whole `@solid-primitives/lifecycle` certification under
  `node` was refused at witness acquisition with a probe contradiction, which
  also took `createIsMounted` and `isHydrated` with it.
- `createPureReaction`'s `onCleanup` registration is unconditional under
  `browser` (the guard never returns), but every proposal and census read the
  guard as a possible early exit, so its owner requirement could only be
  `min: 0`.
- A dead arm was reachable to the census, so any call in it counted as a call
  the export may make.

## Decision

**Under a certification that declares exactly one host, `browser` or `node`,
an import that binds exactly to a module-level boolean constant the host's
resolution fixes is that literal wherever a condition is decided. A host-free
certification, a declaration of two hosts, `deno` or `worker` folds nothing:
both arms stay live.**

### What is proved, and from which bytes

`host_constants_of_module(importer, source, host, boundary)` answers, per
named value import of a bare specifier in the importer:

1. Node's lookup from the importer finds the package, stopping at `boundary`. A
   certification passes its private project's root, the tree the checker
   itself wrote from authenticated snapshots at each package's original
   relative position, so the lookup cannot reach a package an ancestor
   directory happens to hold. The package's `name` must equal the specifier's,
   and a legacy `browser` field or an absent `exports` map folds nothing.
2. `exports` (read with key order preserved; it decides the meaning) selects
   targets for the subpath. Every target the host's resolution may select is
   collected, whatever setting the *unfixed* conditions take (`development`,
   `observe`, a bundler's own): the fixed ones are the host, `import` and
   `default`. A map that may select nothing for some setting, a pattern, an
   array fallback or a `null` target folds nothing.
3. Every one of those targets must export the imported name as a module-level
   `const` bound to a `true` or `false` literal
   (`exported_boolean_constant`; `export const x = …` or a local export clause),
   and all must agree. A `let` or `var`, a written binding, a re-export, a
   star export, a direct `eval` in the module, or any other initializer makes it
   `None`. For `@solidjs/web@2.0.0-rc.9` `isServer` agrees across all three
   files per host (`isDev` does not: `web.js` and `web.dev.js` differ), so
   only `isServer` is folded.

The value is read from the resolved module's bytes, never from its name: a
package that exports `isServer` as `typeof window === "undefined"`
(`@solid-primitives/utils`) folds nothing. The witness a fold rests on lists
every target with its SHA-256 (`ResolvedHostConstant::witness`).

### What it applies to

Only a reference the checker binds to exactly that import. The Rust side
(`HostConstantScope`) resolves an identifier through `oxc_semantic` to the
symbol of the named import specifier, so a shadowing parameter or local, a
namespace member (`web.isServer`), a default import, an import of another
name (`import { isDev as isServer }`), an alias that is written, or any other
module's reference is untouched. The producer (`hostConstantTruthinessLocked`)
resolves the identifier through the checker to an alias declared by exactly one
value `ImportSpecifier` whose imported name and module specifier equal the
constant's, in the importing source file the constant names. A binding
initialised from the import and never written folds through the existing
const indirection; a written one does not. Two contradictory constants for one
import fold neither.

### One fold, three readers

- **The proposal.** For a case that declares a host the emission analyses a
  folded source: each `if` a constant decides keeps its live arm, the
  decided condition is rewritten to the literal it reads as, the dead arm
  becomes `;` (a dead `else` is blanked), and statements after an `if` whose
  live arm always leaves the function are blanked (function declarations keep
  their text, a variable keeps its binding with a `0` initializer, so no
  reference changes the binding it resolves to). Byte offsets and line breaks
  are preserved, so every span is the span of the same text. The generator's
  walks (`returns`, `creates`, owner requirements) then propose without
  knowing about hosts: `onElementConnect` under `node` proposes
  `returns: []` and no cleanup.

  A folded file is still the archive's file. The Type Facts program keeps the
  bytes on disk and joins each file by source digest, so the fold is handed to
  `build_project_native_measured_with_program_hashes` beside the original
  digest, which becomes the fact file's `source_hash`; every fact keyed by span
  joins as before. `source_hash` is also what a resolution record's module
  digest is compared to (`runtime_binding_entity`): a first version that left
  the folded bytes' digest there kept the fold's answers but made every
  export of `@solid-primitives/scroll` under `browser` unattributable
  (`fallback-all`, so each export degraded to `{"call": {}}`), and the
  checkpoint showed it as six exports moving from partial to degenerate. The
  fold is not applied to the Type Facts program (an overlay was tried first;
  it joined, but the digest comparison above failed the same way).
- **The census.** The request carries `hostConstants` (below) and the producer
  reads a decided condition in `literalTruthinessLocked`, which already
  drives branch reachability. A dead arm is `unreachable`; the `if` and `?:`
  arms it leaves live are evaluated whenever the `if` is.
- **The `unconditional` fact.** `callRunsOnEveryCompletion` reads the same
  decisions (an early exit written only in a dead arm is no path out, an arm a
  decided condition selects is evaluated whenever its parent is), so
  `ImplementationCall.unconditional` and the generator's
  `solid_facts::ast::unconditional_calls` (which takes the folds) agree under a
  host. Under `browser`, `createPureReaction`'s `onCleanup` is unconditional;
  under `node` nothing after its guard runs.

### The certification's evidence, and the wire

The certifier does not trust the emission's fold. `private_project_host_constants`
re-derives every constant from the private project's materialized bytes (the
authenticated snapshot of each package) for the modules of the plan and its
dependencies, under each plan's own declared host, and attaches them to the
Type Facts session (`Session::set_host_constants`); every `invocations` and
`export-values` request carries them. The census therefore proves a fact
about the audited archive's bytes, and a proposal made from an unaudited
installation that disagrees is refused at the census.

Handshake protocol **75** adds one optional request field, `hostConstants`:
rows `{importer, specifier, name, value}`. A protocol-73 producer decodes with
unknown fields refused and would reject the request, so the number moves
(74 belongs to a concurrent change). The schema digest, the Go and Rust
constants, and the protocol test move together.

### What stays unfolded (fail closed)

- host free, two declared hosts, `deno`, `worker`;
- a specifier that is not bare; a namespace, default or aliased-name import;
- a package with a `browser` field, no `exports`, a pattern or array target,
  targets that disagree or that a setting of an unfixed condition may fail to
  select;
- a re-export chain (`export { isServer } from …`): it is not a named value
  import of the module that defines the constant, and the resolver does not
  chase it;
- a written alias, a shadow, a module containing a direct `eval`;
- a condition that is not a literal, a never-written `const`, or `!` of one
  (`typeof window`, `a && b`, a comparison of a host constant).

The emission fold rewrites `if` statements only. A `?:` or `&&` on a host
constant is folded by the producer's reachability and `unconditional` reading
but not by the proposal, which then proposes over both arms: fewer claims,
never a wrong one.

## Consequences

- `host-constant-{free,browser,node}` pin the proposal side on the real
  `@solidjs/web@2.0.0-rc.9` `package.json` and `dist/*.js` bytes, with
  `onElementConnect` byte for byte from `@solid-primitives/lifecycle`. Host free
  and both negatives (`shadowedGuard`, `namespaceGuard`) propose alike; `browser`
  moves only `guardedCleanup`'s owner requirement to `min: 1`; `node` proposes
  `returns: []` for `onElementConnect` and nothing for `guardedCleanup`. The
  contract corpus gained a `hosts` map (fixture name to host), read by
  `scripts/contract-corpus.mjs`.
- The producer's tests (`tsgo/host_constants_test.go`) pin the same decisions
  on the census: branch reach, `unconditional`, return-site reach for the real
  `onElementConnect` and `createPureReaction` bodies under each host, and each
  binding rule above (shadow, alias, namespace, other specifier, other name,
  other module, contradictory constants).
- `make primitives-checkpoint`, before and after (release binary, 97 packages,
  721 exports; the "after" figures are in the commit message and the report
  that carries this change).

### Remaining approximations

- Only booleans bound to a literal constant are folded. `isDev` differs across
  the files a host may select in `@solidjs/web@2.0.0-rc.9`, so it is not.
- The resolver reads `exports` conditions the way this repository's
  certification requests them (`import`, the host, `default`); a package whose
  map orders a condition it does not model before them folds nothing rather
  than guessing.
- A dependency the certification does not materialize (not in the plan's
  dependencies, so absent from the private project) is not found, and its
  constants fold nothing.
- The proposal folds `if` statements only (above).
