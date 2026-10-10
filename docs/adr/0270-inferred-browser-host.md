# ADR 0270: Browser host from feasible execution reachability

- Owner decisions accepted, 2026-10-10; implementation and measured review below.
- Owners: native discovery, normalized syntax facts, dialect vocabulary, IR host
  graph, independent contract/solver views, diagnostics and ESLint/Oxlint.
- Extends ADRs 0140, 0220, 0241 and 0269. Replaces prior round-3 assumptions.

## A. Deferred callbacks and feasible execution

A callback registered from browser-executed code with a reviewed host trigger is
browser-executed: it **CAN run in the browser**. A browser-strength defect in code
that can run in the browser is a real defect. This is the same standard as the
existing strict-read/write rules, which already report code that runs only on
some paths. Other server, test or unknown callers cannot cancel that witness.

This premise covers intrinsic DOM event handlers selected by the compiler's
EventHandler fact, Solid control-flow children functions (For, Repeat, Show,
Switch, Match, Loading and Errored) through the dialect, effect compute/apply and
reviewed bundle members, onSettled, onCleanup, reviewed timers and
addEventListener declarations, and receipt-admitted authored deferred Call rows.
Exact direct local parameter/prop invocations retain their actual invoker site.
Constructing an arbitrary function argument or component prop supplies no edge.

Withhold a recognized registration when non-execution is proved: an exact
returned handle is cancelled in the same synchronous block before any task
boundary; the live-branch fact proves the supplying/invoking site client-dead;
a control-flow condition is constant false; the consuming component ignores the
prop or invokes it only server-side; or a server-only/"use server" function
receives it. Empty For inputs and zero Repeat counts also prove no selection.

Cancellation uses normalized ordered call spans and exact standard-library
declarations, handle/binding identity, receiver identity, and client-live
cancellation. Adjacent calls, intervening inert statements, a direct host guard,
and immediately consumed returned handles are supported. Unknown cancellation
is not a non-execution proof. Listener removal also binds event type, callback
identity and omitted/boolean/explicit own-boolean capture mode. Opaque helpers, getters, arbitrary
effectful intervening statements and asynchronous cancellation cannot supply
that proof; the reviewed registration remains feasible.

Feasibility supplies only an execution host. It never proves owner, tracking,
phase, guaranteed delivery, completion, disposal or universal path coverage.
Cleanup delivery cannot discharge an acquired resource merely because disposal
is feasible. Missing exact callback targets or unsupported invocation shapes
retain baseline analysis under the precision contract.
Module-level onCleanup without an ambient owner never registers the supplied
callback and stays baseline. Options-bearing lazy memo demand is still unproved.
Unknown argument evaluation cannot establish that registration was reached;
inert literals/closed objects and exact initialized local callable inputs can.
Cleanup registration also needs a positive owner premise. The bounded host
collector admits an exact immediate inline owner-creating callback; unowned
helpers, fresh-stack callbacks and context-dependent helpers stay baseline.
Literal no-demand computations with `defer: true` cannot activate apply, and
literal inert compute/apply pairs cannot activate an error bundle member.
Other reviewed effect/error triggers keep the feasible-execution premise.

An exact local synchronous call's continuation is client-dead only when a
bounded normal-completion predicate proves CertainExit under authenticated
client constants. Normal return, unknown target/control, recursion, unsupported
inputs, a fourth nested call exceeded, 256 visited nodes exceeded or an 8192-byte
body exceeded yields MaybeNormal/Unknown. That continuation remains feasible
**if the call returns**, which premise A accepts. This is an explicit
overapproximation, not a proof that the function terminates; bound exhaustion
must never become CertainExit. Async calls synchronously return a promise and
generator calls allocate an iterator, so their bodies' throws do not kill the
caller's continuation. Eager argument/callee exits remain separate prerequisites.
Process-exit-like builtins, methods/dynamic calls and external callees are not
modelled termination authorities.

## B. Optional client resolver edges

Default CLI, ESLint and Oxlint inference reads closed configuration data without
executing user config. Local import edges require exact Type Facts attestations
and authored paths naming the same canonical analyzed file.
An alias additionally needs the round-3 single-target congruence rule and the
review's exact-file check. With no resolve.extensions override, tsconfig-path
resolver or admitted plugin rewrite of the local request, ordinary Vite probes
are also allowed: `.mjs`, `.js`, `.mts`, `.ts`, `.jsx`, `.tsx`, `.json`, followed
by directory index probes in that order. The single existing canonical file
must agree with the exact Type Facts target. Competing files/indexes, directory
package.json resolution, extension substitution, bare tsconfig-only aliases,
conflicting aliases and symlinks supply no default edge. Requested resolver
failure still never falls back to these probes.

With `--runtime-resolution required` (ADR 0220), the project's installed Vite
client-environment resolver supplies optional edges, including aliases, and
supersedes the congruence guess. Join exact importer, occurrence span and literal
text; require its file path and realpath to equal the attested analyzed target.
Missing, unknown, external, virtual, query-qualified or disagreeing answers
retain baseline authority. Requested failures never fall back to the guess.
Declaration/runtime package matching still belongs to artifact admission.
Explicit aliases of package imports remain baseline until their exact runtime
export surface is authenticated; local edge answers cannot replace that proof.
Requested CSS resolution must also agree with its reviewed local input.

An Oxlint JS-plugin user opts in with:

```json
{"settings":{"solidChecker":{"runtimeResolution":"required"}}}
```

The ESLint adapter uses the same setting. The packaged launcher/adapter supplies
the ADR 0220 worker; `SOLID_CHECKER_RUNTIME_RESOLVER` can override it and
`SOLID_CHECKER_PROBE_NODE` selects Node. Opt-in executes project Vite config and
client resolver hooks, refreshes native answers on each request and bypasses
adapter/daemon reuse. Default is `"off"`.

ADR 0220 observes serve/development client resolution with Vite 6/7/8. It is not
production-build or transformed-source attestation. The independent bundler
draft's mandatory build graph and second build flag are omitted per decision B;
no build is run automatically. Unknown transformations and unreviewed provider
bytes still refuse root/proof admission.

## C. Roots, baseline and certification

Round-5 static-link gate: every non-type static import and export-from (including
`export *` and empty bindings) must have a loadability premise before module
evaluation has browser authority. Exact local edges require analyzed targets;
bare packages require an exact installed client-entry loadability proof below,
and requested runtime answers must agree with its canonical selected entry.
Missing package/runtime identity is unknown. A `~/` TS-only request under an
empty closed plugin/alias set and proven absent installed `~` package is
provably unloadable. Both outcomes withhold evaluation, including outgoing
call/callback/load edges. Static dependencies propagate refusal transitively;
dynamic import failures withhold their target, not their importer. Generated
route manifests require the exact audited provider and no unproved eager graph.

Package linking has an explicit **published-export premise**: the installed
package's published declarations accurately describe its runtime value exports
for the selected client entry. A missing declared binding is TypeScript's job;
this checker never reports that duplicate claim. This premise applies equally
to named/default imports and export-from, while namespace/side-effect imports
still require an entry. It is a linking premise, not receipt-issued behavior,
byte authentication, completion, ownership, tracking or certification authority.
External initialization remains feasible-if-returning under premise A; local
static dependency completion is still separately required as described below.

Join the exact Type Facts NodeModules row to an importer-relative installed
manifest, matching its resolver package name/version and canonical declaration
root. pnpm/Bun package-store links are supported; an unrelated/shadowing install
cannot answer for the declaration package. Select a public exports key (exact
subpath first, then the most specific single-star pattern), without falling back
to main when exports hides it. Conditional objects retain author key order:
an earlier default is earlier than browser/import. The installed Vite 8.3.0
`dist/node/chunks/node.js` lines 671–680 and 28887–28903 confirm default client
conditions `module`, `browser`, `development|production`, with `import` added
for ESM. The audited Solid provider additionally prepends `solid` in
`configEnvironment` (installed next.44 `dist/esm/index.mjs:4286–4307`). The proof
requires both plain/provider projections in both development/production modes
to resolve; the optional runtime answer must select one of those exact files.
This is conservatively stricter than selecting a single deployment mode.
[Vite conditions](https://vite.dev/config/shared-options.html#resolve-conditions)
and [Node conditional exports](https://nodejs.org/api/packages.html#conditional-exports)
describe the ordering and active conditions; condition-list order is not a
replacement for the package's own key order.

Without exports, use browser/module/jsnext/main and deterministic Vite file/
extension/index probes. A browser remap to false, absent entry, hidden subpath,
declaration/native-only entry or canonical file escaping its package root
withholds loadability. Exports arrays and directory-package resolver branches
remain unknown. Known Solid runtime owners come from
`Dialect::audited_installation`, the single audited triple identity; its current
rc.13 entries receive the audited classification. An installed unaudited release
can still load its client entry under the same premise, while SC9014 and the
dialect's release-specific semantics remain unchanged. The old rc.3 receipt
bundle is neither consulted nor substituted. Provider runtime-owner dedupe also
requires the same root-selected install; explicit user dedupe remains refused.

CommonJS entries are loadable through Vite's normal optimizeDeps conversion to
ESM, including named imports. Authored CommonJS/require in application modules
remains unsupported. A CommonJS package excluded from optimization or linked
outside node_modules (Vite treats it as source) is refused;
unknown browser behavior still receives no package semantics.
[Vite dependency pre-bundling](https://vite.dev/guide/dep-pre-bundling.html)
documents this conversion. No pre-bundling or user code runs during discovery.

A provider-authenticated `virtual:file-routes` request is independently loadable
when codeSplitting is enabled and an on-disk route inventory proves no eager
`route` export or export-star that could supply one. The audited
filesystem-routing `dist/vite/index.js` resolveId/load hooks always provide the
id, but lines 205–226 emit static picked imports for `$$route` or disabled code
splitting. Those shapes remain baseline pending generated-graph/pick authority.
Excluded/unanalysed route files and directory membership are inputs too. A lazy
manifest supplies no page initialization, navigation or body invocation edge.
The ADR 0220 protocol reports virtual ids as Unknown, so opt-in still refuses
them; missing requested answers never fall back to provider inference.

Static dependencies must also have proved normal initialization completion.
Unknown suspension/completion and a certain dependency throw withhold the
importer's evaluation. Independent entry prefixes remain eligible. Circular
static graphs retain baseline pending binding-initialization/evaluation-order
facts; no TypeScript symbol edge overrides a possible TDZ. Opaque evaluation
withholds its module's outgoing authority too, since eval can change call targets.
Ordinary possibly-throwing earlier calls remain feasible under premise A.
Workers and service-worker loads do not supply a window-host edge; iframe/srcdoc
strings and HMR callbacks supply no new root/invocation edge.
HTML meta http-equiv policies also refuse discovery: an authored CSP such as
script-src 'none' can prevent every script entry from executing. A script tag
alone cannot override that policy; ordinary charset metadata remains admitted.

Classic inline scripts with no type or text/javascript execute as separate
elements. A thrown exception ends that script, not the parser's later module
elements, as specified by HTML's separate preparation/execution steps and
per-document deferred-script list.
[HTML script execution](https://html.spec.whatwg.org/multipage/scripting.html#execute-the-script-element)
is the authority. A closed syntax grammar admits scalar locals/conditions,
literal throws, exact localStorage.getItem reads and the reviewed
document.documentElement.dataset.theme assignment. These scripts supply no
analysis roots. document.write/open, navigation/location assignment, window.stop,
script-removal/replacement, eval, aliases/computed calls, loops and unknown calls
refuse. A mere text blacklist would miss aliases and is not an independence
proof. Independent inline modules likewise supply no analyzed root but do not
cancel an external sibling; opaque/interfering inline modules remain refused.
External classic scripts remain unproved, and authored CSP still refuses.

Keep the reviewed round-2/3 plugin closure allowlist and closed config grammar.
Require the exact analyzed tsconfig file in the application directory (its name
need not be tsconfig.json), an admitted Vite configuration
and exact browser root. Explicit target/conditions win; libraries, published
entries and enclosing published packages are never inferred. Server roots never
become browser roots. Client entry probes include .tsrx in the audited order;
an unsupported selected .tsrx refuses inference instead of falling through.
An empty project references list adds no boundary; nonempty references require
an independently complete execution inventory and remain refused.

Importing a route manifest supplies neither page initialization nor default-body
execution. Lazy load/picked-export/feasible navigation and invocation facts are
still required; this implementation refuses manifest-only routes. Exact authored
imports can separately establish initialization, which never invokes a default.
Module/function "use server", nested server closures, orphan RPC-only imports,
middleware, entry-server and client-mode Document retain their baseline.

AstFacts schema 56 normalizes host-sensitive live/dead/unknown predicates,
separate default-initializer extents, exact void-zero inputs, ordered calls and
literal empty/zero values. Oxc nodes stay in solid-facts. import.meta.env.SSR
requires structural identity and admitted Vite semantics; imported isServer
requires exact binder/Type Facts identity and dialect/package/runtime authority.
Unknown predicates, aliases and missing facts never prove deadness or execution.
Vite development injects a mutable import.meta.env object. Env/meta escape,
computed access, direct write/delete/update or unsupported property shapes
withhold the file's execution/completion facts, including outgoing edges and
callback bodies; source can otherwise change SSR before a later host test.
Ordinary direct reviewed scalar metadata/env reads preserve the constant premise.
Lazy component prop/child evaluation, JSX default/destructuring initialization
and continuations after suspension require separate positive premises. Prefixes
in separate statements remain eligible. Exact synchronous local callables
normalize a CallCompletion predicate, instantiated with the same authenticated
host constants as live-branch facts. Direct host/literal tests, simple unwritten
literal parameters, nested exact calls, ordinary returns, throws and stable
while-true loops distinguish certain exit from a possible normal return.
Defaults/destructuring/rest stay MaybeNormal. Nonliteral simple parameters do
not choose control branches; either branch remains possible. Parameters used
only outside control do not manufacture a normal return before a certain exit.
An early normal-return branch survives later throwing statements. Async rejection
is not a synchronous exit. Unsupported class/namespace initialization withholds
later same-block authority rather than assuming successful initialization.
The new ModuleCompletion fact is separate from prefix eligibility. Eager
sequence/argument/declarator ordering propagates bounded certain exits. This
does not prove arbitrary interprocedural termination. Composite eager expressions
without indexed sibling ordering conservatively withhold their whole projection
when their bounded completion is dead; separate-statement prefixes stay eligible.

Build baseline and browser contract/solver views independently. Browser findings
are selected only where primary and contributing authored sites have browser
proof authority. Unselected findings retain the independent baseline. Context
premises, opaque effects, incongruent resolution and indirect callable export
trails retain baseline. Shared callers do not cancel a browser witness.
An existing baseline violation is retained if the selected browser view has no
matching violation; browser closure may resolve a baseline uncertifiable result.

Inference is monotone over today's default finding authority and affects findings
only. Public program/metrics/package summaries remain baseline. It cannot certify
an application: certification requires an explicit target, and inferred clean
results remain uncertifiable. Receipt/contract production never uses inference.

Roots, input bytes/presence, .tsrx probes, scope predicates/refusals, callback
premises/cancellation, fresh runtime answers and both contract views enter cache
identity. Native inputs are revalidated before/after solving. No inference view
can turn a TypeScript diagnostic into a checker-owned violation.

## D. Inference decisions and recall (round 7)

Every completed native diagnostic analysis without an explicit target emits one
`solid-checker: note:` inference decision on the existing stderr note channel.
The daemon retains and replays the same decision on cache hits; ESLint/Oxlint
projects it through `solid-checker/contract-note`. A successful decision names
the admitted scope count and roots. A refusal names its path, source line where
available, and the missing premise. Discovery returns a typed refusal rather
than silently returning None. These notes are separate from findings, status,
certification and contract emission.
Coverage inspection and contract emission report their explicit-condition
acquisition as a refusal to infer; they never acquire inferred authority.

Import use-site reference-space demands were missing: the classifier read a
fact requested only at the import binding. Demand the exact binder-selected
uses too. ReferenceSpace and Oxc runtime_referenced are aggregate observations;
mixed server/client uses retain baseline until per-use client lowering is
known, so a server value use cannot authenticate an outside type use. Direct
export identity can also join a canonical symbol's one exact declaration when
the target has no entity at that binding span. Ambiguous/conflicting entities
and indirect export trails remain refused.

Congruent literal aliases and extensionless imports remain subject to the
round-5 static-link gate. Vite selects the first existing default extension or
index file, not a unique candidate; Type Facts must select that same canonical
file. Later candidates cannot invalidate that deterministic selection. Local
symlink/redirect refusals remain; installed NodeModules store links instead
pass to independent installed client-entry loadability. Package name/version or
a store realpath alone supplies neither entry proof nor behavior authority.

The HTML grammar admits balanced passive SVG shapes with fragment-only use
references. SVG script/integration elements, event attributes, external refs
and unbalanced subtrees refuse. Unproved script interference and source CSP
still refuse; round 8 admits independently executing inline scripts as above.

Read-only inspection of the seven rc.13 corpus apps found all configured
provider closures already pinned, including pnpm/Bun layouts and next.27's
forwarding plugin. Their defineConfig syntax and actual tsconfig identities
already fit the grammar. Error Menu's passive SVG refusal is repaired. All
seven refused in the lead's round-7 native measurements: six first named a
missing core/meta static-load premise, and Jar Hell first named its inline
classic script. Round 8 supplies those linking/independence premises. Read-only
source/manifest predictions now give Error Menu a candidate src/main.tsx root.
Round 9 bounds extraction and supplies inert class and resource linking proofs.
Compass's no-extends analytics classes and Helge's ordinary SVG URL import are
candidate repairs; Error Menu's bare Scalar CSS request now has an exact installed
export/file path. Jar Hell's HttpError still needs exact constructor proof for
its unresolved global Error heritage; syntax/name matching cannot provide it.
Beacon/Lutra/Queue retain eager picked route-config dependencies;
Queue additionally retains its divergent /src alias. Built-in semantics remain
distinct from artifact authentication, and no rc.3 receipt substitutes for rc.13.
These post-patch predictions are not native measurements or scope-count claims.

### Bounded extraction and Vite resources (round 9)

Completion state is shared by the entire file. Exact expression/body spans carry
one cached literal input environment; a different environment gets an unknown
expression or MaybeNormal body, never another specialization's non-return proof.
Exact Oxc declaration nodes replace full AST searches per local invocation and
guard. Write/redeclaration checks are cached per declaration. Each expression is
analyzed once; statement fallthrough and ordered block prefixes are cached, and
execution summaries share ancestor work after applying each child/parent edge.

The global limits are 32,768 completion node visits and 131,072 execution steps;
depth is at most 64, local call depth remains four, body work remains 256 nodes
and 8,192 bytes, and fan-outs are bounded. Predicate trees have at most 64 nodes,
so cloning, evaluating and emitting predicates cannot become quadratic in file
size. Limits use counters, never wall time. Direct execution/expression overflow
is Unknown (both possibilities, no positive host witness). Body refusal remains
MaybeNormal under premise A's CallCompletion wrapper; a skipped branch cannot
leave behind a later throw as certain non-return. Exhausting either global
budget withholds every host witness in the file. Missing constants do not make
a positive execution predicate true, although premise A still admits ordinary
call continuations if returning. The existing wire schema and default baseline
are unchanged. Visit-count and predicate-storage regressions use 2,000 nested
ternaries/logicals and 500 calls of the same body, plus wide symbolic prefixes.

Installed Vite 8.3.0 node.js cssPostPlugin (around lines 29,349-29,390) produces
CSS module maps and injected ordinary styles; assetPlugin (31,818-31,885) emits
default string exports for its known asset extensions, ?url and ?raw. This is
a linking premise only, separate from behavior/certification authority. Literal
local files or exact installed package export entries must exist; all condition
choices must remain files of the requested stylesheet/asset type. Source and
manifest candidates, canonical targets and lookup absence are fingerprinted
before daemon reuse and again at admission. Requested resolver unknown/mismatch
still refuses. No resource supplies a callable, component or initialization edge.

Compiled stylesheets (including CSS module and CSS ?url) now withhold linking:
audited loader identity does not prove successful transformation of arbitrary
input. This refuses @reference, @apply and other unproved closures, including
escaped spellings, without consuming an outside stylesheet. Tailwind discovery
also records its inventory before refusing any stylesheet. Raw CSS remains inert
text; every admitted raw/asset file, package dependency and absent lookup is
recorded by the same resolver at discovery and before daemon reuse. Asset
default/namespace surfaces are admitted; unknown named exports remain unproved.

Nullish coalescing and nullish assignment always compose left completion. With no exact nullish facts,
selection/right completion and tails remain unknown even for normal-return
twins; certainly throwing left calls remain dead. Coverage result reuse is
disabled for all units without an explicit runtime target, because the native
inference closure includes inputs above the materialized analysis location that
coverage's key does not yet enumerate. Explicit-target units retain caching.

Classes with no eager static work, decorators or effectful computed keys complete
normally; instance fields/methods are deferred allocation contents. Absent
heritage is admitted. Identifier heritage additionally needs an exact private,
unwritten local class/function constructor with no prototype mutation/escape;
class bases must precede the declaration and cannot be ambient-only. Imported,
global (including Error), aliased, async/generator and unknown heritage stays
uncertifiable. This is partial class coverage, not blanket class completion.

## Verification and remaining limits

Nineteen small hand-stated consumer documents reuse round 3's eighteen and add
the callback integration fixture. Each registration computation uses
`creates: []`, `computations: ["registration"]`. Published selected signatures
are preserved; synthetic void ownership requirements are not TypeScript errors.
The live Phase19 document pin is 204 -> 223; historical pins are untouched.
The callback fixture passes an in-memory no-emit TypeScript 6.0.3 check against
installed published rc.13 Solid/signals/web typings and real web JSX.

Unsupported callback identities/defaults/destructuring, cross-file prop
forwarding, factories, operation-trigger chains, context specialization,
unreviewed frameworks, SSR/hydration providers and mixed-route lowering remain
fail closed. Compiler-owned function children must actually exist as exact
normalized targets; this ADR does not permit ill-typed JSX to manufacture them.

### Initial implementation measurements (2026-10-10)

The earlier read-only rounds above describe their historical predictions. The
initial implementation audited the installed Tailwind 4.3.3 CSS transform together
with Vite's style injection/export-map lowering. Config-loader imports `vite`
and `vitest/config` were already closure-audited but incorrectly vetoed resource
compatibility; including them removes that accidental refusal. Executable CSS
directives, unknown resource plugins, nested stylesheet imports and mismatched
resolutions still refuse. Independent review subsequently showed that this did
not establish successful transformation: compiled stylesheet linking is now
withheld as described above.

Read-only `import.meta.env?.DEV/SSR` scalar access no longer implies an env-object
escape. Optional tests supply no host constant, and eager optional-chain
completion remains unknown. Compass passes its deferred store.ts inspection but
still refuses analytics/config.ts, which passes the env object to a helper.
No env-alias/interprocedural premise was added.

An adversarial throwing-constructor case exposed a false inferred violation
after `new Stop()`. Construction now withholds tail/module completion until an
exact constructor proof exists, while retaining the executable prefix and eager
argument exits. Normal constructors also remain unknown; this is deliberate
partial coverage. An authorized native fixture and class/function twins pin it.

Import-reference demands request reference space independently of symbols and
runtime identity, preserving every existing fixture finding and location.
Nineteen synthetic fixture packages now have linking-only runtime entries;
their unchanged hand-stated contracts remain the behavioral authority. Only two
new fixture snapshots moved after native review: the live congruent-alias helper
and the deterministic first-extension reachability case. No existing snapshot
changed. Coverage compared 239 projects and 1,583 findings without differences.

Plugin dependency digests are shared within one audit pass only. Large input
manifests are freshly hashed by at most eight scoped workers, preserving order
and absent/unreadable inputs. The independent solver views share one outer
before/after input-validation transaction; standalone analyses retain fresh
validation. No metadata cache, input omission or timing ceiling was introduced.

The 49-project no-target rc.13 sweep against `rc13-head-notarget.json` retained
538 violations and 3,504 uncertifiable findings. There are zero added/removed
violation sites. Helge's one SC9005 package finding changes from absent accepted
contract at column 1 to explicitly unknown router claims at column 10; it stays
uncertifiable. Helge admits 18 scopes from src/index.tsx initialization and adds
no violation. Remaining nominated applications retain the conservative
alias/resource/completion/eager-route refusals recorded in the timing table.

Paired native release runs alternate saved HEAD and patched binaries three
times per app, with daemon reuse disabled. Medians in seconds:

| App | HEAD | Patched | Delta | Decision |
| --- | ---: | ---: | ---: | --- |
| queue-management-ui/ui | 0.2135 | 0.4111 | +0.1976 | Divergent ~/app.css alias |
| error-menu-web/web | 0.7227 | 0.8691 | +0.1464 | ApiTokensDialog initialization unproved |
| jar-hell-web/web | 0.2095 | 0.4076 | +0.1981 | App.css nested stylesheet import unproved |
| beacon-web/apps/web | 0.6723 | 0.8559 | +0.1836 | Eager generated route graph unproved |
| compass-ui/apps/ui | 1.3486 | 1.8443 | +0.4957 | analytics/config.ts env escape |
| helge-dev | 0.0981 | 0.7227 | +0.6246 | 18 inferred scopes; zero added violations |
| lutra-console/console | 0.1226 | 0.2359 | +0.1133 | Eager generated route graph unproved |

Helge is at the upper edge of the owner's approximate 0.1–0.6 s budget, not a
claimed strict 0.6000 s ceiling. An earlier idle run measured +0.5843 s, and an
unpaired final run measured +0.6715 s; retain those variations rather than
selecting only the fastest result. No repository performance gate is relaxed.

The armed diagnostic suite passes all 19 tests. Its refusal assertions preserve
baseline results instead of demanding a new SC9005 for every withheld host;
some controls have release refusals or empty findings, and unowned cleanup
registrations remain baseline violations. Explicit-target positives select the
synthetic registration span. Three rc.9 fixture controls refuse default runtime
selection before native host admission, so their native default results do not
independently establish SSR/library/override coverage; lower-level tests exercise
those boundaries.

Handoff verification is green: workspace Clippy with warnings denied; armed
`make test-rust` (1,735 passed, zero failed/ignored); CLI (381 tests); and full
`make verify` (TOTAL 210.76 s, zero "FAILED during step" lines). The full gate
passes unchanged-snapshot coverage, ownership, contract corpus/conformance,
TypeScript oracle, Go race tests, performance certification, and all 354 script
tests. The known standalone audited-archives environment failure is absent in
the correctly armed full gate. No public contract or bundled artifact was
regenerated. Approximate/fail-closed cases above remain; the gate does not claim
complete host inference or constructor coverage.
