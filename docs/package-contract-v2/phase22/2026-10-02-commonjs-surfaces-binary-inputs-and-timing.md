# CommonJS surfaces, binary inputs and callback timing

This round adds **44 package-invoked callback write-error examples across two
packages**, with successful permitted-write controls for every example. It also
establishes safe delayed callbacks through two Lodash methods and nine fflate
APIs. Five decoder flows restore the exact bytes produced by their matching
encoders. Every executed consumer passes the installed published typings.

There are 420 new browser consumer records. These are finite experiments with
`authority: false` and `certification: false`. No production analyzer rule,
accepted contract or finding snapshot changed.

## What became reachable

The earlier root-export scan admitted no Lodash callbacks. Its declaration
namespace differs from the actual imported value: the published default import
has 307 members, of which 305 are callable. `surface-selection.mjs` asks
TypeScript for the exact imported value and its property symbols, records their
declarations, and generates receiver-preserving member calls.

The sampler distinguishes a published `export =` declaration from an ES module
namespace. It resolves the former through the default import allowed by the
real compiler options. It records the runtime package and the separate typing
package: Lodash 4.18.1 and `@types/lodash` 4.17.25. Both artifacts are pinned by
their bytes. The existing callback selector and historical studies stay intact.

`extended-witnesses.mjs` adds standard typed-array, buffer, view and Blob seeds.
Native seed assignability is checked by TypeScript. A callback that accepts a
numeric size and returns a typed byte array gets `size => new Uint8Array(size)`;
the requested size is honored. There are no assertion casts or parameter-name
recipes. Nano ID's `customRandom` now receives a valid callback of this shape.
This witness tests callback execution, without claiming cryptographic quality.

The selector prefers nonempty array profiles so collection helpers can actually
invoke their iteratees. TypeScript checks every generated candidate before
admission and checks the full instrumented consumers again before execution.

| Retained package | Version | Callable members | Admitted callback APIs |
| --- | --- | ---: | ---: |
| Lodash | 4.18.1 | 305 | 135 |
| fflate | 0.8.2 | 22 | 9 |
| Nano ID | 6.0.1 | 4 | 1 |

There are 146 candidate APIs. Lodash's `pull` is refused because every generated
candidate produces TS2769 against the actual types. The callback-shaped rest
values do not satisfy the published rest argument. The checker gains no rule
for that type error.

## Errors, callers and controls

The two surface chunks contain 290 records for 145 paired APIs. All pairs
execute, 143 read controls are clean, and 55 APIs invoke a generated callback.
The other 90 remain unverified under this construction/return-consumption flow.
They are not classified as safe.

| Observation | APIs |
| --- | ---: |
| Mapped native callback write errors with clean controls | 52 |
| Package as immediate callback caller | 44 |
| Application directly invokes the returned supplied callback | 8 |
| Package examples with successful `ownedWrite: true` controls | 44 |

The package examples comprise 43 Lodash exported names and Nano ID's
`customRandom`. Lodash examples include `map`, `filter`, `reduce`, `forEach`,
`memoize`, `cloneDeepWith`, `update`, `flow`, `times`, `debounce` and `throttle`.
Aliases remain separate exported names; this is not a count of implementation
defects or independent semantic families.

The eight direct application callers are `first`, `head`, `last`, `sample`,
`max`, `min`, `get` and `iteratee`. Their native write errors are real, but they
do not establish that the package invokes the supplied callback. Exact immediate
caller attribution excludes them from package behavior evidence. All 52 caller
classifications have mapped files; none is inferred from an optimized filename.

All 44 package examples have an owner and no public observer in their original
read callbacks. The private native write rule still rejects their writes. The
88 permission-control records change only the native signal's explicit
`ownedWrite: true` option. All 44 write callbacks then execute with an owner and
no warnings, errors or blocked requests.

Two read controls fail on typed but invalid generated domains:

- `bindKey` is given a key whose target method does not exist and throws when the
  returned wrapper is consumed.
- `bindAll` is given a method name whose property is not callable and throws
  during invocation.

These are runtime input failures permitted by the real signatures. They are
excluded from the write-error claim. No contract is manufactured from them.

## Timing changes the permission

The original `debounce` witness uses `leading: true`; the original `throttle`
witness omits options and takes its published runtime's default leading path.
Calling their returned wrappers during the owned construction invokes the
supplied callback immediately and produces the mapped write error.

The four timing-control records retain the same owned caller and default
signal, set `leading: false` and retain trailing delivery. Both supplied
callbacks execute later without an owner or observer, and both write twins
remain quiet. `delay` and `defer` also deliver unowned, permitted callbacks in
the original surface sample.

These observations require execution phase and concrete options in any static
claim. A warning attached unconditionally to `debounce`, `throttle` or every
callback-bearing member would reject these legitimate writes.

## Returning a function does not identify its use

The generic consumer eagerly invokes a returned zero-argument function. fflate's
`AsyncTerminable` instead cancels delivery. Its published documentation says
that termination prevents the callback; `esm/browser.js` implements `cbify` by
posting work and returning `() => worker.terminate()`. Immediate consumption
therefore cancels most of the work this experiment wanted to observe.

`binary-delivery-selection.mjs` and `binary-delivery-cases.mjs` supply the actual
published two-argument callback overload, await delivery, record the supplied
error, and invoke the terminator after delivery. This is an explicitly reviewed
fflate flow, rather than an inferred interpretation of every function return.
All eighteen consumers type-check and all nine APIs deliver callbacks without
an owner or observer. Their writes are permitted.

Four compression operations succeed on the standard `[1, 2]` bytes: `deflate`,
`gzip`, `zlib` and `compress`. The other five callbacks report `unexpected EOF`
or invalid gzip/zlib/zip data. The generic error channel remains quiet because
these errors are delivered as callback values. A clean error channel alone
therefore does not establish a valid package operation. This round's refinement
validator records all five domain failures explicitly.

The decoder challenge replaces those five inputs with package-produced data:

| Decoder | Producer used for this finite flow |
| --- | --- |
| `inflate` | `deflateSync` |
| `gunzip` | `gzipSync` |
| `unzlib` | `zlibSync` |
| `decompress` | `gzipSync` |
| `unzip` | `zipSync` with one named byte entry |

The first ten records establish successful delivery. Ten additional payload
records assert the exact recovered `[1, 2]` bytes, including the named archive
entry. All five read and write twins pass that assertion without diagnostics.
This relation table is package-specific experimental input. The broader
namespace and binary sampler needs no such table, but successful format-specific
flows still require a compatible producer or a reviewed domain fixture.

## Updated picture of scalability

Combining the prior 187 exported names across 67 packages with these 44 additional
package examples gives **231 exported names across 69 sampled packages** with
ownership or package-dependent callback feedback. This is an adaptive sample,
and exported aliases are counted. It does not estimate coverage of most app
imports, most misuse classes or unexecuted paths.

The scalable part is shared semantic invariants. Native ownership and write
feedback applies through third-party code once that code executes, while exact
caller mapping explains which package path delivered it. Published declarations
can cheaply select real receivers and typed inputs across different module
formats. Exact byte pins keep observations attached to their installed artifacts.

Other requirements remain separate:

- Immediate static feedback needs a proven package execution model, exact
  caller phase, concrete option branches and the native signal's write policy.
  The source walker retains zero callback-context assumptions in this sample.
- Runtime feedback observes executed paths. Lazy operators, class protocols,
  providers, private state and complex domains need further consumer flows.
- Function return types alone do not distinguish a producer, wrapper, cleanup
  or cancellation operation. Eager consumption can suppress the very behavior
  being measured.
- Callback error values can leave the exception channel quiet. Their handling
  and the application's success requirements need explicit facts.
- Intended snapshots, background work and desired updates cannot all be
  recovered from code and traces alone; the earlier paired-intent experiments
  still apply.
- SSR, hydration, HMR, other runtime versions and deployment/bundler differences
  remain outside these browser observations.

The evidence supports a development feedback layer centered on shared runtime
invariants, plus narrow static claims when source facts close the required
behavior. It does not establish universal package certification or automatic
knowledge of every API's valid input domain.

## Validation and artifacts

The retained studies are `surface-browser-0`, `surface-browser-1`,
`surface-permitted-browser`, `surface-timing-browser-fixed`,
`binary-delivery-browser`, `binary-roundtrip-browser` and `binary-payload-browser`
under `rust/target/`. Each freezes its local source closure and selection before
and after execution. Historical successful studies remain unchanged.

`surface-validated.json`, `surface-origins.json`,
`surface-permission-controls.json`, `surface-timing-validated.json`,
`binary-delivery-validated.json`, `binary-roundtrip-validated.json` and
`binary-payload-validated.json` retain the individual results.
`surface-controls-validated.json` checks the combined caller, permission, timing,
delivery-error and exact-payload claims.

The initial timing setup refused an absent optional-options slot before any
consumer ran; its failed startup directory is retained. The comparison validator
also had an argument-parsing startup failure before producing a verdict, which
was corrected. No successful observation was replaced and no expectation was
relaxed to turn either setup failure into evidence.

The experiment tests pass 79/79; the new regression checks size-aware byte
callbacks against TypeScript and retains refusal of private nominal shapes.
Syntax checks pass for all 129 experiment modules. `make verify-fast` passes
producer freshness, Rust formatting and pinned workspace Clippy. `git diff
--check`, JSON schema parsing and dialect manifest validation also pass.
Full `make verify`, analyzer coverage, ownership gates and contract certification
are deferred because production semantics and their inputs were not changed.
No published package, installed typing artifact or native runtime was patched.
