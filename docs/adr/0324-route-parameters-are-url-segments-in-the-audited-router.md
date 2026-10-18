# ADR 0324: Route parameters are URL segments in the audited router

- Status: accepted and implemented (2026-10-18). The owner approves premises
  that are faithful to Solid 2.0 and to its audited packages; this is one, on
  the standing of ADR 0295/0300's package-integrity premise.
- Owners: `router_params_status` (`solid-reactive-ir/src/router_params.rs`),
  `Dialect::router_params_are_url_strings` and `audited_router_releases`
  (`solid-dialect`, table `AUDITED_ROUTER_RELEASES` in
  `solid_2/releases.rs`), the `<Show>` and `params.k` primitive proofs in
  `solid-reactive-ir/src/attribution.rs` (`show_child_parameter`,
  `show_accessor_holds_primitive`, `router_param_member`), the premise entry
  and the SC9014 router notice in `solid-facts-backend/src/diagnostics.rs`.
- Relation: extends ADR 0320's primitive-value proofs (`value_is_primitive`)
  with two producers. Nothing here changes what the router does; it states
  what an audited release does and refuses what the audit did not read.

## Context

kui reads its route parameters as `<Show when={params.clusterId}>{(id) =>
<KsqlScreen clusterId={id()} />}</Show>` with `useParams()` from
`@solidjs/router`. The component then hands `props.clusterId` to project
functions that use it in ways no member invocation describes (`fetchBrokers(api,
clusterId)` and the like), which ADR 0320 discharges when the value is a
primitive. Nothing proved `id()` primitive: the checker could not see that a
route parameter is a string. 110 `SC9012 parameter-use-unmodeled` findings in
kui (46 distinct locations; kui compiles through two tsconfig projects) rested
on exactly that.

## The premise

Stated plainly, as the stdlib trust is stated in ADR 0318:

> `useParams()` of an audited `@solidjs/router` release hands out route
> parameters as URL path-segment strings, or `undefined` for an absent optional
> one. A `<Show when={params.k}>` children callback therefore receives that
> string (the accessor form's `id()`, or the value itself under `keyed`).

### Evidence (installed `@solidjs/router@2.0.0-next.26`)

The package resolves two entries, and both reach the same code:
`exports["."].default` is the bundled `dist/index.js`; the `solid` condition is
`dist/index.jsx`, which re-exports `routing.js` and `utils.js`.

| claim | `dist/index.js` | unbundled |
| --- | --- | --- |
| a dynamic segment's value is the location segment string: `match.params[key] = locSegment` where `locSegments = location.split("/")` | `:76` | `utils.js:81` |
| a splat's value is the remainder string `locSegments.slice(-lenDiff).join("/")` | `:85` | `utils.js:91` |
| a filter only accepts or rejects (`matchSegment`); nothing transforms a value | `:50-` | `utils.js:52-` |
| an absent optional parameter has no key (`expandOptionals` yields a pattern without it), so it reads `undefined` | | `utils.js` `expandOptionals` |
| the route's parameters are an `Object.assign` of its matches' params | `:1040-1046` | `routing.js:483-489` |
| `useParams()` returns `useRoute().params` | `:700-702` | `routing.js:163-165` |
| that object is `createMemoObject(getParams)`, a Proxy whose `get` is a memo of `fn()[property]`, unless `utils.paramsWrapper` replaces it | `:1306`, `:113` | `routing.js:725-728`, `:954`; `utils.js:124-` |
| the declared type | `types.d.ts:27` `Record<string, string \| undefined>` | |

`paramsWrapper` is declared in `RouterUtils` (`types.d.ts:285`, with
`queryWrapper`), supplied through a `RouterIntegration`'s `utils`
(`routers/history.d.ts:14`); nothing in the package assigns it.

`<Show>` (`solid-js@2.0.0-rc.13`, `dist/solid.dev.js:1462-1486`): `keyed =
props.keyed`; `conditionValue = createMemo(() => props.when)`; when the
condition is truthy the callback is `child(c)` with `c` that value under
`keyed`, and otherwise `child(() => { if (!untrack(condition)) throw …; return
conditionValue(); })`. Every value a callback sees is the truthy `when` value
itself, with no coercion.

Audit record, `dist` bytes of the read release: `index.js` sha256
`b657bc8dde1275e710dccada942c4d0cd8c7d222e72a4dd0e298f9cce77baa35`,
`routing.js`
`639f16fd285168d4bad002d19029d1eb31c50711577b816af9c9adc98bb153ea`,
`utils.js`
`562b280a63c8af8204a8ec91bbb8956cbcc1b7644d88f0886461c784ebb97e2a`. The
premise is keyed by the resolved `package.json` version under ADR 0300's
integrity premise (installed bytes are the published bytes of that version);
it does not re-hash the install.

## Decision

1. **Audited release table.** `Dialect::router_params_are_url_strings(version)`
   answers for an exact `package.json` version. Solid 2's table is
   `["2.0.0-next.26"]`. The version is the compiler's attested resolution of
   the project's `@solidjs/router` import (`AttestedImport::package_version`
   of a `NodeModules` row whose package name and resolver package name are
   both `@solidjs/router`), never a name or a lockfile. The release alone is not
   enough: the installed bytes must hash to the audited digest recorded beside
   it ("The installed bytes", below).
2. **Status.** `router_params_status` is, per project: `NotUsed` (no file
   imports `useParams` by name from the router), `Applies` (every such import
   resolves to an audited release and the project replaces nothing the audit
   read), `Unaudited` (a resolved release the table does not list, or an
   import whose resolution was not attested), or `Replaced` (below). Only
   `Applies` applies the premise.
3. **Producers.** Two primitive producers, both inside ADR 0320's
   `value_is_primitive`:
   - the callback parameter of a `<Show when={E}>`: `id()` (accessor form) or
     the parameter itself (`keyed` literal flag) holds a primitive when every
     truthy value of `E` is one;
   - `params.k` for the direct binding `const params = useParams(...)` of a
     named, non-type-only import from the router: a string or `undefined`, so
     its truthy values are strings.
4. **Notice and premise.** An `Unaudited` project carries an `SC9014`
   uncertifiable notice (rule `unaudited-solid-release`, context
   `router-release`) at the first `useParams` import, naming the resolved
   release and the audited one. An `Applies` project lists the premise in the
   output's `premises`; every other project's output is unchanged.

## Refusals

The premise is not applied where the audit did not read the code that runs:

| refusal | where |
| --- | --- |
| the release is not audited: in the corpus `2.0.0-next.16` (donegeon-client), `next.17` (compass-ui), `next.18` (app-game, beacon-web, error-menu-web, solid-groove) and `0.15.3` (kobalte-docs) | status `Unaudited`; the apps that import `useParams` by name (beacon-web, error-menu-web, solid-groove) gain the notice, the others are `NotUsed` |
| any file outside the router's own spells `paramsWrapper`, `queryWrapper`, `wrapParams`, `RouterIntegration` or `RouterContext` | status `Replaced`, whole project (the spelling layer; it is not what the premise rests on, see "The history actually passed") |
| some router creation names a `history` that is not provably stock (an object literal, a call of another function or package, a `const` the project uses beyond `get`/`set`, an exported `const`), a `createRouter` that is not a direct call of a closed literal config, a router reached through a re-export, a namespace member that is not a hook, a default import, a deep or dynamic or CommonJS load, a `RouterContext` | status `Replaced`, whole project |
| the specifier resolves outside `node_modules` (a `paths` mapping to a project file), the bundler's own resolution (when the host asked) loads a different package, or the resolved path is a pnpm `patch_hash` install | status `Replaced`, whole project, no notice (there is no router release in question) |
| the installed bytes are not the audited digest (any patch, fork, tarball, `link:`, workspace package, hand edit) | status `Unaudited { bytes_differ }` and the notice, whole project |
| a record names a replacement: a patch line in a manifest or lockfile, a `patches/` entry, an override or dependency that is not a plain registry version | status `Replaced`, whole project (the cheap layer) |
| two router copies at different versions | each import resolves its own `package.json`; one unaudited copy leaves the whole project `Unaudited` (notice at its `useParams` import, else its first import) |
| `useParams` is wrapped, aliased, re-exported or reached through a namespace import | `router_param_member` accepts only the direct `const` of a named import's call |
| `params` is `let`, or an alias (`const a = params`) | the binding must be an immutable identifier bound to the call itself |
| a computed key; a name of `Object.prototype` (`constructor`, `toString`, `__proto__`, `hasOwnProperty`, `valueOf`, …): a route that lacks the parameter falls through the merged object to it | `router_param_member` |
| a name some assignment writes anywhere, or some reflective definition defines (the ADR 0314 census) | `member_may_be_reassigned_on_builtin`, `property_may_be_defined_reflectively_as` |
| `useSearchParams` (its values are strings or arrays, and with a schema argument any type) | not a `params` member: no producer |
| a `keyed` that is not a boolean literal, a spread attribute on the `<Show>`, a second `when`, more than one child or a callback with other than one plain, never-reassigned parameter | `show_child_parameter` |
| an expression that is not exactly `params.k` or a proven primitive (`params.a ?? params.b`, `params.k!`) | `truthy_value_is_primitive` |

How many corpus apps and rows stay open: every app on an unaudited release.
Of the apps that import `useParams` by name that is 3 (beacon-web,
error-menu-web, solid-groove); no finding of theirs was settled before and
none is lost. donegeon-client, compass-ui, app-game and kobalte-docs do not
reach `useParams` by name, so the premise has nothing to decide there.

## The history actually passed (round 12)

The spelling refusal was a layer, not the premise: `createRouter({ history })`
takes the integration's `utils` from `config.history` (`routers/factory.jsx:141`
`utils: history.utils`, `:187` `config.history.utils.renderPath`, `:207`
`staticIntegration(props.url, config.history && config.history.utils)`), and
`createIntegration` hands `utils.paramsWrapper` to the context
(`index.js:1306`). A key assembled at run time
(`["params", "Wrapper"].join("")`) or a history a dependency builds
(`{ ...memoryHistory(), utils: { ...h.utils, paramsWrapper } }`) bypassed every
spelling and got the premise while `params.id` was an object. The premise is
therefore decided from the history each router creation passes:

> The premise applies only when **every** router creation in the program is a
> direct call of `createRouter` (a named, non-type import of the audited
> package) whose single argument is a closed object literal and whose
> `history` is absent, or a direct call of `browserHistory`, `hashHistory` or
> `memoryHistory` (named imports of the same package, `routers/index.js`;
> `routers/history.js:91,131,178` set `go`, `renderPath`, `parsePath`,
> `beforeLeave`, never `paramsWrapper`), or a never-exported `const` initialized
> by such a call whose every other reference is a `get(...)` or `set(...)`
> call. Anything else is a history the checker cannot read.

The package exports no other entry that builds an integration (`index.d.ts`:
`createRouter`, `defineRoute(s)`, the three histories, `RouterContext`);
`RouterContext` is refused by name because a project could provide its own
value. `useParams` and the other hooks read the context and build nothing, so
a namespace import is allowed to reach only hooks. A dependency that creates a
router of its own is not read here: its bytes are the integrity premise's
(ADR 0300), the same trust the audited release has.

kui: its shell creates the router with `createRouter({ routes, base })`, no
`history` (`packages/shell/src/routing/routes.tsx:203`), and every test
harness passes `memoryHistory(at)` directly or through a `const` it only
`get`s and `set`s: kui still qualifies, and the corpus A/B is 0 added, 0
removed against the build before this rule.

## The installed bytes (round 13)

The release string is a claim, and a `patch-package` patch (no manifest or
lockfile entry), a fork installed under the same version (`pnpm.overrides`
mapping the router to `github:...`), an npm `overrides` tarball, a `link:` or a
workspace package named `@solidjs/router` all keep it while the bytes differ.
The record-based refusals guess at install mechanisms; the premise therefore
verifies the **bytes the import resolves to** against the audited digest and
treats a mismatch like an unaudited release.

**What is covered, exactly.** The digest is sha256 over, in order:

1. the line `solid-checker router bytes v1`;
2. for each of `name`, `version`, `main`, `module`, `exports`, `type` of the
   installed `package.json`, the line `<key>=<value>`, the value being the
   field's JSON with object keys sorted at every depth and no whitespace
   (`undefined` when absent): the fields the resolution consults;
3. for each regular file under the package's `dist/`, recursively, sorted by
   its path relative to `dist/` (UTF-8 bytes, `/` separators): `file\0<path>\0`,
   the content length as a little-endian `u64`, then the content. A symlink or
   other non-file entry under `dist/`, or an unreadable one, gives no digest.

That is every file the package's `exports` can load, the declarations the
compiler reads, and the entry fields; the README and LICENSE are not covered
(nothing runs them). `scripts/router-digest.mjs <package directory>` computes
it (Node, no dependencies) and the backend computes the same value
(`installed_bytes.rs::package_digest`; the unit test
`the_published_router_hashes_to_the_audited_digest_and_a_changed_byte_does_not`
asserts the fixtures' published copy hashes to the recorded one). The digest of
the audited release, computed from the registry install the audit read (13
corpus installs of `2.0.0-next.26` all hash alike, and their `dist/index.js`
is the audited `b657bc8d...`), is recorded beside the version in the dialect
table, `AUDITED_ROUTER_DIGESTS`:

| release | digest |
| --- | --- |
| `2.0.0-next.26` | `e67af5395e06b46fba92846736c3efa5f2076281e05e864dda248d96689204d6` |

**Where it is computed.** The backend, which has the resolved install:
`router_install_evidence` hashes the directory of the `package.json` each
router import's attested resolution consulted (`AttestedImport::package_manifest`;
for a pnpm or `link:` install that is the realpath-normalized location, so a
symlinked install is hashed where it lives). The IR reads no disk: it receives
the digests by manifest path in `RuntimeEnvironment::router_install` (skipped on
the wire, so a caller with no disk, such as the WASM adapter, carries none and
the premise stays unapplied: fail closed) and compares each with
`Dialect::audited_router_digest(version)`. A mismatch, an unreadable install or
a missing digest is status `Unaudited { bytes_differ: true }`, with the same
`SC9014` notice: `@solidjs/router 2.0.0-next.26 is installed, but its installed
bytes differ from the audited release; ...`. A release that is not audited at
all keeps the original wording (there are no audited bytes to differ from).

**Version fields.** The version the check consults is `package_version`, the
`version` of the manifest nearest the resolved file, which is also the manifest
the digest hashes. The resolver's own record (`resolver_package_version`) must
not disagree with it: two answers about which package loaded is none, so a
disagreement is unaudited. An unnamed nested manifest (`dist/package.json`)
fails the `package_name` test the same way.

**Retained daemon.** `dist/index.js` is no program source, so the daemon
resolution witness (`daemon_resolution.rs`: program files and each consulted
`package.json`) does not see an edit to it. The daemon therefore keeps the
resolved router roots of its last analysis and hashes each with the same digest
as a cached-answer input (`daemon.rs::input_files`), and the premise status
is in `DiagnosticIdentity` and the cross-file proof digest. Twin
`an_installed_router_byte_edit_moves_the_premise`: one appended byte turns the
premise off, restoring it turns it on, retained equals one-shot at both
retention levels, and with the daemon's router input removed the twin fails
(checked by mutation).

**The record layer stays, cheaply.** `replacement_recorded` refuses on:
a patch line naming the router in `package.json`, `bun.lock`,
`pnpm-lock.yaml`, `pnpm-workspace.yaml` or `yarn.lock` on any ancestor
(`patch:`, `patch_hash`, `patches/`, `.patch` within three lines of the name); a
`patches/` or `.yarn/patches/` entry whose name contains `solidjs` and `router`
(`patch-package` leaves exactly that); an `overrides`, `resolutions` or
`pnpm.overrides` entry for the router, or a dependency on it, whose value is
not a plain registry version (no `:`, `/`, `#`, not a path: `github:`, `git+`,
URLs, `file:`, `link:`, `npm:`, `workspace:`). Each has a fixture whose router is
the published bytes, so the refusal is the record's own.

Yarn Plug'n'Play has no `node_modules` router: the compiler resolves nothing,
the import is unattested, and the status is `Unaudited` (release not
resolved), no premise (`router-params-pnp`).

Residual, stated: a patch applied only at bundle time by a Vite plugin (the
bytes on disk are the published ones), a bundler alias that exists only in
`vite.config` when the host supplied no bundler resolution, and any other
rewriting of the router after install are the integrity premise's (ADR 0300,
AGENTS.md "Dependency source and pins"); the digest closes everything that
changes the installed files.

## Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| a segment value is a string | `params.k` holds a primitive | `match.params[key] = locSegment`, no transform (table above) | `RouteAccessor` |
| an absent optional parameter is `undefined`, not an object | only truthy values are claimed | `Show` hands the callback the truthy `when` value | `RouteAccessor`, `RouteKeyed` |
| `id()` is the `when` value | accessor form | `child(() => … conditionValue())` | `RouteAccessor` |
| the keyed value is the `when` value | keyed form | `child(c)` | `RouteKeyed` |
| `Show` is the dialect's | no lookalike | `lookup.primitives(file).jsx` resolves it | n/a (unit of the resolution) |
| the object is the router's own `useParams()` | direct const of a named import | import row for `@solidjs/router`, callee declared by that import | `WrappedUseParams`, `NamespaceUseParams`, `AliasedParams`, `ReassignedParams` |
| nothing replaced the object | no `paramsWrapper` and friends | `index.js:1306` | `router-params-replaced-wrapper`, `router-params-replaced-integration` |
| no history supplies `utils.paramsWrapper` | every router creation's history is absent or stock and untouched | `factory.jsx:141,187,207`, `history.js:91,131,178` | `router-params-stock-history` (settled), `router-params-history-obfuscated`, `router-params-history-package`, `router-params-history-escaped`, `router-params-history-reexport`, `a_history_edited_in_another_file_moves_the_premise` |
| the specifier is the installed package | attested `NodeModules` row, bundler agrees when asked | ADR 0300 | `router-params-alias-project-file` |
| the bytes are the published ones | the digest of the manifest fields and `dist/` equals the audited one | `installed_bytes.rs`, `scripts/router-digest.mjs` | `router-params-modified-bytes`, `an_installed_router_byte_edit_moves_the_premise`, the unit test of the digest |
| no recorded replacement (cheap layer) | no patch line, `patches/` entry, non-registry override or dependency | the install-mechanism records | `router-params-patched`, `-patch-package`, `-patch-directory`, `-override-fork`, `-override-tarball`, `-linked-dependency`, `a_recorded_patch_of_the_router_moves_the_premise` |
| a package manager that has no `node_modules` | an unresolved import is no release | `AttestedImport` | `router-params-pnp` |
| each copy is audited | every router import's own resolved manifest | per-row `package_version` | `router-params-two-releases` |
| the release is the audited one | exact version from the attested resolution | ADR 0300 integrity premise | `router-params-unaudited-release`, `a_router_release_moved_between_audited_and_unaudited_is_resolved_again` |
| the key names an own parameter | not an `Object.prototype` member, not written or defined by the program | the merged object's prototype chain | `PrototypeName`, `WrittenName`, `DefinedName` |
| the value is the callback's only parameter | one plain parameter | `child(c)` / `child(accessor)` | `DynamicKeyed` (a refused flag) |
| a cached answer is current | the release is in the compiler's resolution, so the facts generation and the daemon's resolution witness move with it | ADR 0319 round 5 | the retained twin, both retention levels |

Self-review against the bug families found so far:

- **Key/receiver/origin identity.** The origin is the import row the compiler
  attested for the specifier and the declaration the callee resolves to, never
  the name; the key is the property's own source text, never computed.
- **Visible references only.** The premise reads values, not references: a
  write to `params.k` stores on the Proxy's target and the `get` trap ignores
  the target, and the name census refuses a key some assignment writes anyway.
- **Script files.** The binding must be an immutable `const` of a module's
  import; a script's top-level global is not read.
- **Span equality.** Peeled spans only through the facts' parenthesis rule; a
  type assertion on the way refuses (`value_is_primitive`'s existing rule).
- **Nested function inheriting a name; `__proto__`/`super`.** `__proto__` is on
  the refused names; the callback must be the `<Show>`'s sole child function.
- **Skippable awaits, primitive-by-cast, destructured parameters.** A cast on
  the way refuses; a destructured or defaulted callback parameter is not the
  accessor parameter shape.
- **Merged alternatives, forwarding, shared literals.** Only the exact value
  `when` carries is claimed; forwarding goes through the existing tag census.
- **Mixed roles, window inheritance, views, wrappers, escaping helpers.**
  Not touched: this is a value-domain fact, not a role fact.
- **Retained-session staleness.** The release moves without any source file
  moving; the twin edits the installed `package.json` between audited and
  unaudited and back under a retained daemon (both idle retention levels):
  retained equals one-shot, the unmodeled use is settled only on the audited
  release, the notice appears only on the other, and an unchanged repeat is a
  cache hit.

Round 12 adds to the self-review: **forwarding via aliases/spreads/wrappers**
(a config that is not a closed literal, a spread, a `history` that is a call of
anything but a stock import, an alias of `createRouter`, and a `const` history
passed to any function are all refused, because each reference must be a direct
call of a stock import or a `get`/`set` call); **quoted/numeric keys** (a
config key that is not a static identifier makes the literal not exact, so it
refuses); **`__proto__`** (a config mentioning it refuses); **shared/mutated
literals** (a `const` history referenced anywhere but its config or a
`get`/`set` call refuses, which includes `history.utils = ...` and
`Object.assign(history, ...)`); **visible references only** (the references
come from the binder's resolution; a reference it did not resolve is a
reference the project cannot make through a name); **merged alternatives**
(`cond ? memoryHistory() : custom` is not a call of a stock import); and
**retained-session staleness** (the status joins the cross-file proof digest,
and the daemon watches every patch-record file on the ancestor walk; both
twins pass at both retention levels).

## Consequences

49-project corpus against `/tmp/sweep-final17.json` (4,210 uncertifiable, 554
violations): **−110 `SC9012` uncertifiable** (all kui: 46 distinct locations in
the Alerts, Clusters, Consumers, Group, Messages, Schemas, Topics, Connect and
Ksql routes) and **+4 `SC9014`** notices (beacon-web, error-menu-web,
solid-groove, and en-passant, whose router did not resolve), violations
unchanged: 4,210 to 4,104. With the router rule disabled the same build
removes 0: every removal depends on the premise. Two of the prototype's 112
rows are not removed: a `props.groupId` use in the second kui project, because
that program writes a member named `groupId` somewhere (the name census).

Residual: `Match`'s accessor, `useSearchParams` and `useLocation` values,
`createMemo(() => params.k)` accessors, and a component rendered from
non-router callers are not covered (no producer), so their uses stay open;
kui's ~54 remaining `parameter-use-unmodeled` rows are those, plus uses whose
tags pass constants exported from another file (a separate slice, ADR 0325).
