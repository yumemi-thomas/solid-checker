# Decisions

Every design decision that still governs the code, by ADR number. Code cites
these numbers; link one as `docs/decisions.md#adr-NNNN`. A new decision gets the
next number (0330 onward) and a short entry here: the rule as it holds, its
premise, and the owning files. History, alternatives and review rounds go in
the commit message. The original ADR texts are in git:
`git show docs-archive-2026-10-19:docs/adr/<file>`.

Retired numbers (their subsystems are gone; see the archive tag): 0001, 0004, 0005, 0006, 0007, 0008, 0009, 0010, 0011, 0012, 0013, 0014, 0015, 0016, 0017, 0018, 0019, 0020, 0021, 0022, 0023, 0024, 0025, 0026, 0028, 0029, 0030, 0031, 0032, 0033, 0034, 0035, 0036, 0037, 0038, 0039, 0040, 0041, 0042, 0043, 0044, 0045, 0046, 0047, 0048, 0049, 0050, 0051, 0052, 0053, 0054, 0055, 0056, 0057, 0058, 0059, 0060, 0061, 0062, 0063, 0064, 0065, 0066, 0067, 0068, 0069, 0070, 0071, 0072, 0073, 0074, 0075, 0076, 0077, 0078, 0079, 0080, 0081, 0082, 0083, 0084, 0085, 0086, 0087, 0088, 0089, 0090, 0091, 0092, 0093, 0094, 0095, 0096, 0097, 0098, 0099, 0100, 0101, 0102, 0103, 0104, 0105, 0106, 0107, 0108, 0109, 0112, 0113, 0114, 0115, 0116, 0117, 0118, 0119, 0120, 0121, 0122, 0123, 0124, 0125, 0126, 0128, 0129, 0130, 0131, 0133, 0134, 0135, 0137, 0139, 0140, 0142, 0143, 0144, 0145, 0146, 0147, 0148, 0149, 0150, 0151, 0152, 0154, 0155, 0156, 0157, 0158, 0159, 0162, 0163, 0164, 0165, 0167, 0168, 0169, 0170, 0171, 0172, 0173, 0174, 0175, 0177, 0178, 0179, 0180, 0181, 0182, 0183, 0184, 0185, 0186, 0187, 0188, 0189, 0191, 0197, 0198, 0207, 0208, 0223, 0224, 0226, 0227, 0229, 0230, 0231, 0232, 0233, 0234, 0235, 0236, 0237, 0238, 0239, 0240, 0241, 0242, 0243, 0244, 0245, 0246, 0247, 0249, 0250, 0251, 0252, 0253, 0254, 0255, 0256, 0257, 0258, 0259, 0260, 0261, 0262, 0263, 0264, 0265, 0267, 0271, 0272.

<a id="adr-0002"></a>
## ADR 0002 — Type Facts lives in this repository
The Go Type Facts producer and the Rust process/session client are part of this repository and keep their versioned process seam. A semantic fact is added, produced, consumed and measured in one change. TypeScript-Go internals never cross into Reactive IR: consumers see the wire facts only. Owners: apps/solid-typefacts, rust/crates/typefacts, schema/.

<a id="adr-0003"></a>
## ADR 0003 — The Solid 2 compiler follows `solidjs/solid` next
Solid 2 compiler facts come from `solidjs/solid/packages/compiler` through the `yumemi-thomas/solid` fork at one exact commit. The fork carries semantic-fact models, output-neutral recording hooks, validation, serialization and fact tests only: no change to lowering, output, diagnostics, runtime behaviour, features or performance. A compiler defect is fixed upstream separately and the affected fact stays open until the fork rebases. Owners: rust/Cargo.toml (`solidjs-compiler` rev), rust/dialects/solid-v2/compiler/src/lib.rs.

<a id="adr-0027"></a>
## ADR 0027 — The built-in Solid runtime comes from the dialect
Behaviour of `solid-js`, `@solidjs/signals` and `@solidjs/web` (the dialect's primitive-defining packages) is stated by the selected dialect's reviewed premises, combined with exact symbols, Type Facts and control flow; no package document is read for them. Missing native behaviour stays unknown, never "no effects". Packages with similar names are external. Owners: rust/crates/solid-dialect (`primitive_defining_packages`), rust/crates/solid-facts-backend/src/package_requirements.rs.

<a id="adr-0110"></a>
## ADR 0110 — The checker analyzes Solid 2 only
The resolved `node_modules/solid-js` major picks the dialect. A major this build carries no dialect for (1.x, any future major) yields one project-level `SC9013` refusal naming the deciding `package.json`, and no other findings; a missing or unparsable install defaults to 2.0. The 1.x-only rules `jsx-no-undef` (SC8005) and `prefer-classlist` (SC8013) were dropped without replacement. The eslint-plugin-solid parity corpus is retired: `upstream_compat` is retained behaviour with no upstream control, and a divergence worth keeping is pinned as a product-owned ownership case. `Version::V1` stays so the refusal can recognise 1.x. Owners: rust/crates/solid-facts-backend/src/dialect.rs, rust/crates/solid-dialect/src/lib.rs.

<a id="adr-0111"></a>
## ADR 0111 — Dialect words belong to the dialect
Shared code asks the `Dialect` vocabulary rather than hardcoding primitive lists: whether a call returns a destructurable reactive tuple (`returns_reactive_tuple`) or a store (`returns_store`) is the dialect's answer, so `createOptimistic` and `createOptimisticStore` are recognised without a shared list. A shared `match` on `Primitive` is a seam a new dialect re-enters through; `Primitive` variants no carried dialect names are not live arms. The contract-word half of the original ADR is retired. Owners: rust/crates/solid-dialect/src/lib.rs, rust/crates/solid-reactive-ir/src/{indexes,local_access,source_discovery}.rs.

<a id="adr-0127"></a>
## ADR 0127 — Audited, defaulted and conservative vocabularies are separate
`Solid2::AUDITED` (the audited triple), `Solid2::DEFAULTED` (a project with no resolved `solid-js`) and `Solid2::CONSERVATIVE` (an owner that did not resolve, answered by each field's `#[default]`) coincide today but are kept apart, so moving the audited release changes only `AUDITED_INSTALLATION` and the vocabulary `AUDITED` names. A release the reviews did not read gets the conservative answers plus the `SC9014` notice; older read releases keep their answers and the notice that they are older than the audited release. Superseded in its release number by ADR 0194. Owners: rust/crates/solid-dialect/src/solid_2/releases.rs, rust/crates/solid-facts-backend/src/{dialect,release_scope}.rs.

<a id="adr-0132"></a>
## ADR 0132 — A relative edge inside one installed package is no package boundary
In the package-requirement census a relative import is skipped when the importer and the resolved file have the same owning manifest (nearest `package.json` above each that declares a `name`). The edge stays a requirement when it climbs into another installation, reaches a nested copy of the package, or has an unresolved edge or unreadable manifest. Only part 1 of the original ADR is live. Owner: rust/crates/solid-facts-backend/src/package_requirements.rs (`is_package_local_edge`).

<a id="adr-0136"></a>
## ADR 0136 — `createComponent(C, props)` is a call edge to `C`
The dialect states which call renders a component argument (`Dialect::renders_component_argument`; Solid 2 answers argument 0 for `createComponent`). A call is a render site of project function `F` only when the callee resolves into a primitive-defining package under a rendering name and argument 0 is a bare identifier whose symbol is `F`; anything else stays an escape. Only the attribution call graph reads the edge, kept apart from `function_call_sites` (component identity, execution roles). Owners: rust/crates/solid-dialect/src/solid_2.rs, rust/crates/solid-reactive-ir/src/{indexes,attribution}.rs.

<a id="adr-0138"></a>
## ADR 0138 — `Dynamic` renders the component its `component` prop holds
The dialect lists `(export, prop)` renderers (`component_prop_renderers`; Solid 2: `Dynamic`/`component`) and says only `createMemo`'s accessor yields exactly its compute (`accessor_yields_only_its_compute`). `component_value_flows` follows a value into such a prop only when every reference is a proven read (parentheses, `?:`, `??`/`||`/`&&`, `const X = value` read as `X`, a one-argument parameterless arrow memo read as `X()`); parameters, member reads, `let` and other calls remain escapes. The call graph walks the render as an edge. Owners: rust/crates/solid-facts/src/ast/component_value_flow.rs, rust/crates/solid-reactive-ir/src/{indexes,attribution}.rs.

<a id="adr-0153"></a>
## ADR 0153 — Context roles are a dialect fact (live part)
`Dialect::context_role` names `createContext` (Create), `useContext` (Read) and `createComponent` (Render). Reading `useContext(C)` yields `C`'s default or a value stored under its fresh id; a provider stores its `value` prop. Accessor provenance uses the roles to prove a parameter is only read as context. The package-context census chapters are retired. Owners: rust/crates/solid-dialect/src/{lib,solid_2}.rs, rust/crates/solid-reactive-ir/src/accessor_provenance.rs.

<a id="adr-0161"></a>
## ADR 0161 — A call that runs on every normal completion
`unconditional_calls` returns the calls a function runs exactly once on every normal completion: each step from the call up to the body must be evaluated once, unconditionally, and no earlier statement of an enclosing block may leave by `return`/`break`/`continue`. `synchronously_unconditional_calls` is the caller-facing form (ADR 0275, 0276): an earlier `throw` is an exit, an `async` function counts only calls before its first suspension, a generator states nothing. Under a declared host the conditions are already folded (ADR 0166). Owners: rust/crates/solid-facts/src/ast/unconditional_calls.rs; read by solid-reactive-ir cleanup.rs and execution_role.rs. (The original ADR's proposal-input use is dormant.)

<a id="adr-0166"></a>
## ADR 0166 — A host constant is folded per declared host
With exactly one declared host (`browser` or `node`), a named import of a bare package whose `exports` map selects only targets that all export the name as a module-level `const` bound to the same `true`/`false` literal (`@solidjs/web` `isServer`) is that literal. The value is read from the resolved bytes, never from the name; a `let`, re-export, pattern or array target, disagreeing targets or two hosts fold nothing. The fold is a span-preserving rewrite that blanks the dead arm; the fact file keeps the original `source_hash`. ADR 0295 extends it with Window-realm comparisons. Owners: rust/crates/solid-facts-backend/src/{host_constants,dependency_program}.rs, rust/crates/solid-facts/src/ast/host_constants.rs.

<a id="adr-0176"></a>
## ADR 0176 — An owner requirement after an `await` is proven unowned
A call listed in the producer's `calls_after_await` (after an `await` on every path of its async function's own body) runs from a promise continuation on a fresh stack. Its owner is the dialect's `fresh_stack_callback_owner` answer (Solid 2: none), so an effect, cleanup or owner-requiring call there is proven unowned, with the evidence "runs after an `await`, from a promise continuation". `onSettled`'s returned cleanup keeps the enclosing context; `runWithOwner` callbacks are nested closures and unaffected. Owner: rust/crates/solid-reactive-ir/src/owners.rs (`AwaitContinuations`).

<a id="adr-0190"></a>
## ADR 0190 — A primitive receiver dispatches to its built-in prototype
A parameter-member invocation at a call site carries no `SC9012` obligation when the argument is a primitive literal (not spread), or when Type Facts resolves the call to one standard-library declaration owned by `String`, `Number`, `Boolean` or `BigInt`. Object-typed built-ins (`Date`, `Array`, `Map`) are not covered. The demand plan therefore asks for the resolved call of argumentless member calls rooted at an enclosing function's parameter. Premise: no program patches a primitive prototype to read reactive state. Owners: rust/crates/solid-reactive-ir/src/interproc.rs, rust/crates/solid-facts-backend/src/demand_plan.rs (`parameter_rooted_member_call`).

<a id="adr-0192"></a>
## ADR 0192 — A built-in call in a leaf scope is resolved
The demand plan also asks for the resolved call of an argumentless method call written inside a leaf-owner callback (found by `callback_semantics_at(..).owner == Leaf`), so a standard-library method there is settled rather than an obligation. The demand stays scoped to leaf callbacks and parameter-rooted calls: demanding every member call adds a second symbol candidate and loses proven violations. Optional calls stay open (no resolved call from the producer). Owner: rust/crates/solid-facts-backend/src/demand_plan.rs.

<a id="adr-0193"></a>
## ADR 0193 — An application is the whole program
With no explicit program boundary, the nearest `package.json` decides: `"private": true` is an application (closed); otherwise a manifest publishing `exports`, `main`, `module`, `types`, `typings` or `bin` is a library (open); a manifest publishing nothing is closed; no or unreadable manifest is open. An explicit `--program-boundary`, the ESLint `programBoundary` setting or `.solid-checker/runtime.json` wins. The default is part of the diagnostic identity. Closing removes only the unseen-caller assumption; callers still have to be enumerated exactly. Owner: rust/crates/solid-facts-backend/src/diagnostics.rs (`default_program_boundary`).

<a id="adr-0194"></a>
## ADR 0194 — The audited Solid 2 release is rc.13
`AUDITED_INSTALLATION` names `solid-js`, `@solidjs/signals` and `@solidjs/web` at `2.0.0-rc.13`; its vocabulary is rc.9's (`Solid2::AUDITED` is `RC13`). `READ_RELEASES` is the explicit list rc.0-rc.9 and rc.13; rc.10-rc.12 and rc.14+ get conservative answers under `SC9014`, and rc.0-rc.9 get the older-release notice. The dialect owns the `solid-js/internal` subpath, where rc.13 declares `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`. The rc.13 negative-row chapter is retired. Owners: rust/crates/solid-dialect/src/solid_2/releases.rs, rust/crates/solid-dialect/src/exports/solid_v2_solid_js.rs.

<a id="adr-0195"></a>
## ADR 0195 — A source tracked before the await is a dependency
SC1002 (`reactive-read-after-await`) skips an after-`await` accessor call when the async function has a straight-line call of the same symbol before its first suspension of any kind (any `await`, `for await`, `await using`). `straight_line_calls` excludes `if`, ternary, `&&`/`||`/`??`, loops, `try`, `switch` and optional chains, and is per innermost function. Member reads (store paths, props) keep the rule. A mistake can only hide a finding. Owners: rust/crates/solid-reactive-ir/src/static_rules.rs (`tracked_before_first_suspension`), rust/crates/solid-facts/src/ast/mod.rs (`straight_line_calls`).

<a id="adr-0196"></a>
## ADR 0196 — The compiler follows Solid rc.13
The compiler fork commit, the adapter's upstream, implementation and distribution revision literals, the compiler-facts identity and the identity document move together. Current pin: distribution `3ad4bbec`, upstream `5efaf260`, implementation `c04c4877`; trace version 3, compiler-facts protocol 2. Universal/Dynamic and authored TSRX trace requests refuse. Owners: rust/Cargo.toml, rust/dialects/solid-v2/compiler/src/lib.rs, docs/compiler-facts.md.

<a id="adr-0199"></a>
## ADR 0199 — A callback prop forwarded to an event runs on dispatch
A function literal written as a project component's prop takes `ExecutionRole::EventCallback` when that component only hands the prop to DOM event dispatch. The tag must resolve to one project function; the props parameter is one identifier whose every reference is a static member access that is an intrinsic event-handler value, inside such a handler, or a prop of another component satisfying the same proof. Follows `merge`/`omit` views, `const` aliases and un-namespaced `on...` spreads; a props object that escapes whole or a forwarding cycle proves nothing. Unproven stays uncertifiable. Owner: rust/crates/solid-reactive-ir/src/execution_role.rs (`forwarded_event_prop_role`, `prop_reaches_only_events`).

<a id="adr-0200"></a>
## ADR 0200 — A literal a wrapper runs as a tracked compute is tracked
A function literal passed whole to an exactly resolved synchronous project wrapper takes `ExecutionRole::TrackedJsx` when every reference to the wrapper's parameter is a direct call inside one function literal that is itself the whole argument of a tracked primitive callback (`createMemo`/`createEffect` compute) in the wrapper's body. Wrappers of wrappers and calls from nested functions stay unproven. Owner: `rust/crates/solid-reactive-ir/src/execution_role.rs` (`forwarded_tracked_compute_role`).

<a id="adr-0201"></a>
## ADR 0201 — A helper's read in its own body runs during the call
At a call with one target (a synchronous project function, outside JSX), an attributed read keeps the call's own role when it is written directly in the callee's body (not in a nested function, timer, listener, getter, `untrack` callback or default parameter) or is the accessor argument the body calls. Reads propagated from a deeper callee stay attributed; ADR 0204 extends the chain. Owner: `read_is_direct` in `rust/crates/solid-reactive-ir/src/interproc.rs`.

<a id="adr-0202"></a>
## ADR 0202 — Feedback reports what the user can act on
An unresolved member dispatch raises no obligation at a call whose execution role is an event callback, deferred callback, tracked JSX or deleted code, since no read rule reports there. The violations / "Needs review" / "Analysis coverage" output tiers now exist only behind `--internal` (ADR 0328). Owners: `interproc.rs` (dispatch roles), `rust/crates/solid-facts-backend/src/snapshot_emission.rs` (`render_default`).

<a id="adr-0203"></a>
## ADR 0203 — A closed program has no unseen caller of an exported helper
In a closed program, the `exported-parameter-member-dispatch` obligation of an exported helper is discharged when every reference to it or its aliases is the callee of a call resolving to it, its declaration, an import, or an export specifier. JSX renders, rendering props, values handed out, class members and unnamed functions keep it. Open programs are unchanged. Owner: `discharge_closed_program_export_dispatch`, `entered_only_through_call_expressions` in `rust/crates/solid-reactive-ir/src/attribution.rs`.



<a id="adr-0204"></a>
## ADR 0204 — Three reads that run while a component renders
(1) A read written directly in a function entered through a chain of plain identifier calls (no methods, at most eight hops, cycles prove nothing) is direct. (2) A callee's default-parameter accessor read runs at a call omitting that argument, without spread. (3) A literal that is exactly a component prop runs during render when the child calls `props.<prop>` directly in its body. "Directly in a body" excludes nested parameter lists. Owners: `directly_entered_functions` (`rust/crates/solid-reactive-ir/src/interproc.rs`), `prop_literal_invoked_during_render` (`execution_role.rs`).
<a id="adr-0205"></a>
## ADR 0205 — Coverage gaps group by family and subject
Each analysis-coverage finding carries `coverageFamily` and `coverageSubject`, set at projection from the defect's fields; grouping never parses message text. Seen only in internal views (ADR 0328). Owners: `coverage_group` in `rust/crates/solid-reactive-ir/src/projection.rs`; `render_coverage` in `rust/crates/solid-facts-backend/src/snapshot_emission.rs`.

<a id="adr-0206"></a>
## ADR 0206 — A component's owner carried to runWithOwner is not null
The owner argument of `runWithOwner` is proven non-null when it names a `const` initialized by a dialect-resolved `getOwner()` call written directly in a proven non-async component body, not in a nested function or default. Parameters, `let` bindings, properties and helpers stay conditional; disposal is a separate question. Owner: `component_owner_binding` in `rust/crates/solid-reactive-ir/src/owners.rs`.

<a id="adr-0209"></a>
## ADR 0209 — A method of an exact instance is followed in a leaf scope
A method call in a leaf-owner callback is followed when the receiver is a `const` bound to `new C(...)` (or `this` in such a method), `C` is a single-declaration project class, and the resolved method is declared in `C` itself. No assignment may write a member of that name or replace a prototype (ADR 0213/0314 narrow this). Owners: `exact_instance_method` in `rust/crates/solid-reactive-ir/src/cleanup.rs`; `member_name_may_be_reassigned` in `indexes.rs`.

<a id="adr-0210"></a>
## ADR 0210 — Host callbacks and member calls in a leaf scope
A standard-library call's arguments follow the audited timing table: inline callbacks (`forEach`, `map`, `call`/`apply` receiver, setter updaters, `Promise` executor) are walked like helper bodies; fresh-stack and deferred callbacks run after the scope; `PromiseLike.then` and unaudited callables leave SC9012 open, never a violation. Member calls are read from the resolved declaration, never the span's entity. Synchronous dispatch is handled by ADR 0329. Owners: `member_call_operations`, `standard_library_argument_operations` in `cleanup.rs`; `runtime_semantics.rs`.


<a id="adr-0211"></a>
## ADR 0211 — A value whose origin fixes its class fixes its members
An argument that is a fresh built-in (array literal; `new` of Array/Date/Map/Set/WeakMap/WeakSet/RegExp/ArrayBuffer/DataView/typed array; `Array.from`/`of`, `Object.keys`/`values`/`entries`, `String.split`; species-built array results) runs prototype members that read no reactive state; an instance of exactly a project class runs that class's own plain method. Followed through `const` only; void where the program may replace the member. Owners: `value_origin`, `class_method_symbol` in `rust/crates/solid-reactive-ir/src/indexes.rs`.

<a id="adr-0212"></a>
## ADR 0212 — An accessor's value is proven from every write
Proven built-ins also include: a signal accessor whose init is proven and not a function, every setter reference a call and every write or updater return proven; a `createMemo` accessor with a synchronous literal; a plain call of an exact project function with every return proven; `??`/`||`/`&&`/`?:` of proven operands; array literals; members of a proven receiver (`sort` returns it; `map`/`filter`/`slice`/`split` build fresh values). Props, stores, cross-file accessors stay open. Owners: `accessor_origin`, `setter_writes_builtin`, `returns_builtin` in `indexes.rs`.
<a id="adr-0213"></a>
## ADR 0213 — A parameter holds what every caller passes
In a closed program a parameter-member obligation on a plain unwritten identifier parameter is discharged when the function is entered only through calls (ADR 0203) and every call's argument (or default), followed up to four hops, is a proven origin. A named prototype write (`C.prototype.m = f`) refuses only `m`; replacing a prototype or writing at an unknown key refuses all. Owners: `CallGraph::parameter_holds_builtin` in `rust/crates/solid-reactive-ir/src/attribution.rs`; `member_name_may_be_reassigned`.

<a id="adr-0214"></a>
## ADR 0214 — A prop holds what every tag passes
In a closed program an obligation on `props.name` is discharged when `props` is the component's sole plain parameter, never assigned; the component is rendered only through resolved JSX tags (a call, `createComponent`, rendering prop or other escape fails); and every tag passes a built-in value (a spreading tag fails, a missing attribute is `undefined`, `children` is not followed). Owners: `prop_holds_builtin`, `rendered_only_through_jsx`, `value_holds_builtin` in `attribution.rs`.



<a id="adr-0215"></a>
## ADR 0215 — The value-flow proofs survive review
Computed writes record their literal or `const` key; any other computed prototype write refuses every member; a binding is written if any assignment target or loop head references it. An exact class instance has the class's own members only if no constructor returns a value and no field, parameter property or computed element can define the member, through the whole `extends` chain (unresolved proves nothing). Function values must be current. Owners: `class_instance_is_exact`, `function_value_is_current`, `binding_written` in `rust/crates/solid-reactive-ir/src/indexes.rs`.
<a id="adr-0216"></a>
## ADR 0216 — A forwarded prop is as reactive as its source
`<Inner value={props.value}/>` makes Inner's prop reactive, static or unresolved exactly as the caller's own prop is, by a least fixpoint over forwards; a prop is static only when no chain reaches a reactive or unresolved value. A props object passed whole, `merge`/destructured views, a store passed whole, several children and `props.a.b` off a static head are unknown, never reactive. Owners: `classify_passed_expression`, `resolve_forwarded_props` in `rust/crates/solid-reactive-ir/src/source_discovery.rs`.

<a id="adr-0217"></a>
## ADR 0217 — Leaf scopes reach helpers, namespaces and more host callbacks
Argumentless calls in helpers a leaf callback reaches get a resolved-call fact. `ns.f()` on a namespace import binding follows the exactly resolved exported function declaration or `const` function; computed members and aliases keep the obligation. Host rows: `Promise` executor and `NodeList.forEach` inline; global `addEventListener` and `MediaQueryList.addListener` deferred; `MediaSession.setActionHandler` fresh-stack. Owners: `rust/crates/solid-reactive-ir/src/cleanup.rs`, `runtime_semantics.rs`; `rust/crates/solid-facts-backend/src/demand_plan.rs`.



<a id="adr-0218"></a>
## ADR 0218 — A callback every use of which is closed is not a strict read
A callback handed to a project function or component clears only the SC1001 finding when every runtime reference of its subject (parameter, optionally one static property of an exact literal or attribute) is closed: a dialect tracked slot, exact unlabelled `untrack`, compiler-tracked JSX, a fresh-stack scheduler, or an exact forward. Storing, aliasing, unknown packages, DOM listeners and cycles are open; labelled `untrack` and `callback_may_open_strict_window` slots never inherit a window. Owners: `rust/crates/solid-reactive-ir/src/project_consumer.rs`, `projection.rs`; `Dialect::callback_may_open_strict_window`.
<a id="adr-0219"></a>
## ADR 0219 — A type query and a namespace call are accounted entries
The entry census of ADR 0203 also accounts for erased type queries (`typeof fn` in type position) and a namespace call whose property resolves to the call edge's declaration, only while no runtime code reaches the module as a namespace object (escaping namespace import, `export * as`, dynamic `import()`/`require`, transitively). Specifiers resolve by the compiler's module graph; unresolved bare ones may name anything; files outside the program import no project module (ADR 0193). Owners: `module_namespace_escapes`, `specifier_reaches` in `attribution.rs`.

<a id="adr-0220"></a>
## ADR 0220 — The project's bundler answers where a load goes
Opt-in `--runtime-resolution required` runs a Node worker that loads the project's own Vite config and asks a client plugin container (`resolveId`) about each load occurrence, joined by literal span. With the table present `specifier_reaches` uses it alone; unknown answers (error, virtual or query module, declaration target, `require`) may reach anything; any failure leaves an empty table; the run bypasses the daemon. Default off. Owners: `rust/crates/solid-facts-backend/src/runtime_resolution.rs`, `packages/cli/scripts/runtime-resolver.mjs`.

<a id="adr-0221"></a>
## ADR 0221 — A prop head and an async prefix run at the call
The head of a props member chain (`props.snapshot?.x`) is a Get backed by the ADR 0216 fixpoint. A control-flow primitive's sole exact children literal runs while rendering. An exact plain call of an async project function runs its prefix up to the first suspension (no suspending loops or `switch` tests). An omitted argument activates its default. New async/default paths need a runtime source witness; a store Get needs a key the dialect says warns. Owners: `local_access.rs`, `interproc::body_site_runs_during_call`, `Dialect::store_key_warns_strict_read` (exact rc.13).
<a id="adr-0222"></a>
## ADR 0222 — A returned source and a helper's prop read reach the caller
A project function whose every normal completion returns the same unwritten `const` bound to a dialect source primitive in that frame (exact, current, synchronous, block-bodied, no loops/`try`/`switch`) returns a fresh source per call. A component-local helper's prop read counts at each proven exact plain call with the call's role and ADR 0216 backing. Owners: `rust/crates/solid-reactive-ir/src/source_discovery.rs`, `local_access.rs`; `completion_return_cover` in `rust/crates/solid-facts/src/ast/completion_call_cover.rs`.

<a id="adr-0225"></a>
## ADR 0225 — An exact receiver for getter instantiation
A getter on an argument runs at the call that copies it only when the receiver is a fresh closed object literal, or one immutable local `const` initialized by one (no alias, export, descriptor change or earlier spread; no internal spread, computed or numeric key, duplicate, setter or `__proto__`). Anything else stays an explicit obligation. Only the receiver rule is live (used by ADR 0283); the contract protocol words are gone. Owner: `rust/crates/solid-reactive-ir/src/property_gets.rs`.

<a id="adr-0228"></a>
## ADR 0228 — The certified contract tier is retired
`bundledContracts`, `installedPackages` and `exportConditions` stay in the WASM `CheckRequest` and its typings for compatibility but are accepted and have no effect. Owners: `rust/crates/solid-checker-wasm/src/lib.rs`, `packages/wasm/index.d.ts`.

<a id="adr-0248"></a>
## ADR 0248 — Contract gaps are opt-in in ESLint
No shipped ESLint config enables the `package-contract-incomplete` rule; it is filtered out of every config's default-enabled set. Uncertifiable results are anyway not user-facing (ADR 0328). Owner: `packages/cli/eslint.cjs` (`CONTRACT_GAP_RULE`).

<a id="adr-0266"></a>
## ADR 0266 — Standard runtime configuration premise
Assumed by default, vetoed only on a positively identified exact runtime reference in analyzed source: a call of `enableExternalSource`, or any value reference to `DEV` or `OBSERVE` (a named import used only in truthiness tests does not veto). Identity is the exact declaration name-node span the dialect lists, never spelling; type-only uses and unresolved references never veto. A veto turns every violation into an uncertifiable one project-wide; no third state. Owners: `rust/crates/solid-reactive-ir/src/runtime_configuration.rs`, `rust/crates/solid-dialect/src/solid_2.rs`.


<a id="adr-0268"></a>
## ADR 0268 — Standard hydration host
A fourth clause of the standard runtime configuration premise: hydration runs on Solid's standard serializer runtime, with no custom `sharedConfig.load`/`has` or other host callback. Any positively identified runtime reference to `sharedConfig` (via `solid-js/internal` or the server declarations) vetoes, except direct reads of `hydrating`/`done` and truthiness tests; standard `enableHydration`/`hydrate`/`render`/`renderToString` never veto, and the main-entry `sharedConfig` has no declaration identity so stays Assumed. Owner: `rust/crates/solid-reactive-ir/src/runtime_configuration.rs` (`HydrationHost`).
<a id="adr-0269"></a>
## ADR 0269 — A browser-target config for client applications
The ESLint adapter ships a `browser-<dialect>` config (`browser-v2`): the dialect config plus `settings.solidChecker.runtime.target: "browser"`, opt-in and overridable by a later `runtime` setting. The user, not an inference, declares the target for client apps. Owner: `packages/cli/eslint.cjs`.



<a id="adr-0270"></a>
## ADR 0270 — Browser host from feasible execution reachability
With no explicit target, discovery infers a browser host from closed, non-executing config (one conventional `vite.config.*`, scripts naming Vite only in plain forms, no custom resolver) and client roots; an unproven premise refuses with a `solid-checker: note: browser host not inferred` line. Premises: conventional config, successful build, app starts. A callback registered from browser-executed code with a reviewed trigger can run in the browser unless non-execution is proved. Inference is monotone, never certifies, and an explicit target wins. Owners: `rust/crates/solid-facts-backend/src/inferred_host.rs`, `host_config.rs`; `solid-reactive-ir/src/hosts.rs`.
<a id="adr-0273"></a>
## ADR 0273 — Transported member stability premise
Members of an object a factory returns or a context provides are assumed never reassigned, deleted or redefined. Only a visible write vetoes: any static member write by name on any receiver, `Object.defineProperty`/`assign`/`Reflect.set` by exact library identity, with unknown keys or spread arguments vetoing every member. Two states per member (Assumed, Vetoed). Over-veto only withholds. Owner: `MemberStability` in `rust/crates/solid-reactive-ir/src/accessor_provenance.rs`.

<a id="adr-0274"></a>
## ADR 0274 — Option getters through a forwarding helper
A call of a local helper counts as the copy of its parameter when the helper (exact, current, synchronous, directive-free module, never naming `arguments`/`eval`) passes the plain unwritten parameter to the copying call as the whole value of a `const` or unconditional `return`, outside classes, JSX and fragments. Every getter of a hazard-free prefix (none mentions `this`, `super` or `eval`) is guaranteed. Owner: `rust/crates/solid-reactive-ir/src/property_gets.rs`.

<a id="adr-0275"></a>
## ADR 0275 — A helper's operation counts only on every completion
In the leaf-scope walk, a called body's operation (or call to the next helper) is performed by the call only when it runs on every normal completion the caller can continue from; an earlier `throw` is a way out, an async helper counts only its prefix before the first suspension. Otherwise it is an uncertifiable `leaf-owner-forbidden-call`. The leaf callback's own body keeps the statically-present-code premise. Owner: `reached_on_every_call` in `rust/crates/solid-reactive-ir/src/cleanup.rs`.

<a id="adr-0276"></a>
## ADR 0276 — A read through a helper counts only on every completion
A completion site is a call, `new` or member expression in a function's straight-line flow (no conditional, logical right operand, logical assignment, loop, switch or try; nothing before it can `return`/`break`/`continue`/`throw` out; in an optional chain only what precedes the first `?.`). A summary row is direct only when proven on every completion at every hop; others stay uncertifiable "only on some of its paths". Owners: `AstFacts::completion_sites` in `rust/crates/solid-facts/src/ast/mod.rs`; `site_on_every_completion`, `read_is_direct` in `interproc.rs`.

<a id="adr-0277"></a>
## ADR 0277 — A callback runs only when its helper calls it on every completion
A helper invokes its parameter during the call only when a call of the parameter is a completion site of its synchronous body; this governs inline-arrow roles, stored-literal proofs and accessor-by-reference reads. A call on some paths is an uncertifiable obligation (SC2001 stays silent); `cb?.()` is not a completion site. The completion-call table is bound into `cross_file_proof_digest`. Owners: `invokes_parameter_during_call` in `rust/crates/solid-reactive-ir/src/execution_role.rs`; `parameter_called_on_every_completion` in `indexes.rs`.

<a id="adr-0278"></a>
## ADR 0278 — Exact callback mapping and never-returning guards
A call TypeScript resolves to a `never`-returning signature is an exit; sites after it are not completion sites. A written parameter is not the caller's callback. An argument reaches its parameter only through an exact mapping: no `f.call`/`apply`/`bind`, a current function value, no earlier spread. Owners: `project_callee_at_parameter` in `execution_role.rs`; `never_returning_call_before`, `parameter_completion_calls` in `rust/crates/solid-reactive-ir/src/indexes.rs`.

<a id="adr-0279"></a>
## ADR 0279 — Assertion and never signatures, and exact method callees
The Type Facts producer states, per resolved call, whether the selected signature carries an `asserts` predicate or returns `never`; either makes the call a possible exit (a throw in an undeclared helper is outside the model). A method callee runs its declared body only for a module namespace member, an unwritten member of a `const` object literal, or a member of a `const` bound to `new C()` of the declaring class. Owners: `Call.AssertionSignature`/`NeverReturns` in the producer and `rust/crates/typefacts`; `member_runs_declared_method` in `execution_role.rs`.


<a id="adr-0280"></a>
## ADR 0280 — A call's arguments decide its helper's branches
A call proves parameter values only over an exact mapping with no spread: literals, callable types, omitted arguments. The walk decides `if`/`?:` tests and `&&`/`||`/`??` right operands over `typeof p`, truthiness and null/undefined comparisons into guaranteed, possible or unreachable sites, for the leaf walk's first hop, a called helper's own reads and by-reference accessors; results record a `FunctionSource` dependency. Owners: `argument_site_standings`, `rust/crates/solid-reactive-ir/src/argument_values.rs`; `Entry::CalledWith` in `cleanup.rs`.
<a id="adr-0281"></a>
## ADR 0281 — A store member read inside an argument is no escape
When ADR 0222 checks a returned store root, a reference inside a call argument escapes only if the member chain is the bare root, is itself an argument, is a callee, or is returned by a function; a chain consumed where written (statement, initializer, operand in a callback) is not an escape. Owner: `member_chain_escapes` in `rust/crates/solid-reactive-ir/src/source_discovery.rs`.


<a id="adr-0282"></a>
## ADR 0282 — Tuple slot closures through destructuring
`const [a, start] = make(); start()` names the factory's closure when the pattern is a `const` array pattern of plain identifiers over one direct call and `make` is an exact synchronous project function with one unconditional return of a hole-free array literal whose slot is an inline arrow or an unwritten `const` arrow it declares. Held tuples refuse. A visible mutation of the array iteration protocol makes the slot uncertifiable. Owner: `Resolver::tuple_slot` in `rust/crates/solid-reactive-ir/src/accessor_provenance.rs`.

<a id="adr-0283"></a>
## ADR 0283 — Getter reads through a parameter
A project function Gets `p.key` on every call when it is exact, current, synchronous, directive-free, never names `arguments`/`eval`, `p` is a plain unwritten identifier parameter whose every reference is a static non-callee member read, and the member is a completion site. At a call whose argument is an ADR 0225 exact receiver mapped exactly (ADR 0278), the receiver's getters run, counting only their own every-completion accessor calls. Owner: `parameter_get_sites`, `getter_reads` in `rust/crates/solid-reactive-ir/src/property_gets.rs`.

<a id="adr-0284"></a>
## ADR 0284 — Dependency source mode
`--dependency-source <specifier>` analyzes an installed package's runtime source as project code. Entry selection reads `exports` as Vite with the Solid plugin does for the declared target and build (first of `solid`, `import`, `module`, `default` and host conditions; else `mainFields`); patterns, arrays and `browser` maps refuse. Relative static imports close to 4,000 files in a generated sibling tsconfig; declaration imports redirect to the runtime export within the same package; package files fold under the declared host and their own findings are not reported. Owners: `rust/crates/solid-facts-backend/src/dependency_sources.rs`, `dependency_program.rs`.
<a id="adr-0285"></a>
## ADR 0285 — A forwarded parameter is called where its target calls it
`f` calls its parameter `p` on every completion when `g(…, p, …)` is a completion site of `f`, passes the identifier `p` itself (unwritten, exact position, no earlier spread) to a function declaration `g` that calls that position on every completion, to a fixpoint. `const` callees are refused (temporal dead zone); a module with direct `eval` adds nothing; no destructured parameter may precede `p`. Owner: `parameter_completion_calls` in `rust/crates/solid-reactive-ir/src/indexes.rs`.

<a id="adr-0286"></a>
## ADR 0286 — A typed member root requires an unreplaced slot
Before the typed-source fallback admits a property of an object literal as a source, every read of that property symbol must be Closed: a `const` receiver whose references are all unwritten static member reads, initialized by an object literal (or an exact factory returning a fresh one) with that data property. Escaped receivers refuse silently; for interface-declared members only an Escaped read refuses. Owner: `LiteralMemberSlots` in `rust/crates/solid-reactive-ir/src/literal_member_slots.rs`.


<a id="adr-0287"></a>
## ADR 0287 — A forwarding proxy reads its store
`new Proxy(root, handler)` counts as the store `root` when `Proxy` is the unbound global, the handler is an in-place literal with one non-computed `get` trap returning `Reflect.get(target, prop[, receiver])` on every normal completion (no other trap, `this`, `arguments`, `eval`), and `root` is a store; a same-file function returning only such calls forwards (four hops). Owners: `proxy_get_forward_target` in `rust/crates/solid-facts/src/ast/proxy_get_forward.rs`; `proxied_target` in `rust/crates/solid-reactive-ir/src/source_discovery.rs`.
<a id="adr-0288"></a>
## ADR 0288 — Host-dead return sites belong to the fold; the tuple veto is narrowed
For selected package code the host fold is the only place a host constant decides a branch (project files stay unfolded). ADR 0282's tuple closure is vetoed only by iteration-protocol or prototype-spelled reflective writes, not by every wildcard `Object.assign`. Owners: `fold_emission_host_constants` in `rust/crates/solid-facts-backend/src/host_constants.rs`; `tuple_slot` in `accessor_provenance.rs`.

<a id="adr-0289"></a>
## ADR 0289 — A Get of a package-defined reactive getter is an obligation
In dependency-source mode a project Get (`a.b`, literal computed key, destructuring) in a role that reports untracked reads, whose receiver derives (through bindings, returns, object data members, providers of an exact context) to a getter in package source that calls an accessor, is an uncertifiable SC9012 with context `dependency-source-getter`, never a violation; ADR 0300 adds a separate proof. The context census follows runtime redirects. Owners: `dependency_getters` in `accessor_provenance.rs`; `dependency_getter_obligations` in `interproc.rs`.

<a id="adr-0290"></a>
## ADR 0290 — A package's owner requirement is reported at the project's call
An owner requirement located in package source (not after an `await`) is walked back along edges carrying the reporting bit; inside packages only inherited bits are followed, and each project call that enters the package function becomes the requirement, located at that call with the package operation as evidence. Proven-unowned bits can give a violation, others an obligation; the operation must be on its function's every-completion path (ADR 0275/0280). Owner: `attribute_dependency_requirements` in `rust/crates/solid-reactive-ir/src/owners.rs`.

<a id="adr-0291"></a>
## ADR 0291 — Every Solid package is analyzed from source
`--dependency-source '*'` selects, from each importer, packages whose dependencies or peers name the dialect runtime or a dialect ecosystem scope, that resolve under `node_modules`, are not the dialect's own runtime and are analyzable; others are skipped with a note and stay unanalyzed. `exports` patterns resolve as Node does; relative imports use Vite's extension order. Owner: `dependency_sources::prepare_all` in `rust/crates/solid-facts-backend/src/dependency_sources.rs`.

<a id="adr-0292"></a>
## ADR 0292 — A `lazy` module's `default` is rendered, not retained
A loader's returned `default` component is rendered, not retained, when `lazy(async () => ({ default: C }))` (one argument, rc.13 vocabulary) returns a closed object literal whose `default` names a proven component declared in the loader, and the wrapper is an unexported `const` used only as JSX tags; no prototype writes. The component's body then keeps its rendering role. Owners: `Dialect::renders_loaded_default_argument` (`lazy_renders_component` release digit); `lazy_default_render_witnesses` in `owners.rs`.

<a id="adr-0293"></a>
## ADR 0293 — A store Get warns by its key
A store member chain's warning is its head Get's: a known string key the dialect does not exempt warns; `then` (the only exempt key, `store_key_may_skip_strict_read`) is exempt; symbol, dynamic or exempt-then-chained keys are undecided, an SC9012 `store-key-undecided` obligation, never SC1001. Function members stay violations. Summary rows keep only warning heads. Owners: `store_head_get` in `rust/crates/solid-reactive-ir/src/local_access.rs`; `Dialect::store_key_may_skip_strict_read`.

<a id="adr-0294"></a>
## ADR 0294 — A type is not a reactive source
A declared type (`Accessor<T>`, `Signal`, `Store`) never supplies the premise that a read is reactive. A typed entity is a proven root only through a proven source of the same kind; a visible literal function is decided by its own body where the call is followed; anything else is a typed candidate, and its read in an untracked-reporting role is an SC9012 `type-only-source` obligation, never SC1001/SC1002. Owners: `typed_implementation`, `TypedSource` in `rust/crates/solid-reactive-ir/src/source_discovery.rs`; `local_access.rs`.


<a id="adr-0295"></a>
## ADR 0295 — A declared Window realm decides `typeof window` in package source
Opt-in `--runtime-realm window` (needs a browser target and dependency sources) declares that selected package modules run in an HTML Window realm: `typeof window`/`document` compared with `"undefined"` by `===`/`!==` folds, through `const` aliases and, with an initialization proof, module-level `var` aliases in modules outside import cycles. Declared, never inferred; other forms stay live. Owners: `fold_host_constant_branches_in_window_realm` (`rust/crates/solid-facts/src/ast/host_constants.rs`); `outside_import_cycles` (`dependency_sources.rs`).
<a id="adr-0296"></a>
## ADR 0296 — One preparation path for dependency-source programs
`prepare_program` returns the original tsconfig, host, selection and the sibling tsconfig's exact bytes; one-shot and daemon code share it and `dependency_program.rs` (fold and bind). Daemon retention of source checks is superseded by ADR 0311. Owners: `rust/crates/solid-facts-backend/src/dependency_sources.rs`, `dependency_program.rs`.



<a id="adr-0297"></a>
## ADR 0297 — Source is the default under a declared host
An ordinary analysis declaring both `--runtime-target` and `--runtime-build` with no explicit selection runs as `--dependency-source '*'` when a Solid Vite plugin resolves from the project; otherwise it prints a note and analyzes no package from source. Selection assumes Vite with the Solid plugin reads `exports`. Non-ordinary modes keep their selection; `--no-dependency-source` is a removed flag. Owners: `defaults_to_dependency_sources` (`rust/crates/solid-facts-backend/src/main.rs`); `effective_dependency_selection` (`check.rs`).
<a id="adr-0299"></a>
## ADR 0299 — Calls of a stable exported object's own function
`timer.useTicker()` on a named import of a package's `const` object resolves to its exact function when the object is a closed literal (optionally `Object.freeze`d: no spread, computed or numeric key, duplicate, accessor, `__proto__`), every value use of the receiver is an immediate member call of a function that cannot expose it, and ADR 0273's written-name index does not veto the member. Optional calls need both links proven non-nullish; generators refuse. Owner: `rust/crates/solid-reactive-ir/src/object_members.rs`.


<a id="adr-0300"></a>
## ADR 0300 — The package integrity premise
Code outside the analyzed program neither provides a selected package's exported contexts nor rewrites objects a selected package creates. Every visible provider of an exact default-less package context must supply a proven getter value; visible computed writes and reflective wildcards must prove an unrelated target, else veto. The proof supplies reactive origin only. Reports list this premise in `premises`. Owners: `rust/crates/solid-reactive-ir/src/accessor_provenance.rs`, `interproc.rs`; `rust/crates/solid-facts-backend/src/diagnostics.rs`.
<a id="adr-0301"></a>
## ADR 0301 — Apply messages describe tracking
SC1001 still reports an untracked effect-apply read when its paired compute reads the same source, but its message says the effect can re-run and Solid warns in development, not that it never updates; only exact immutable getter values and unconditional synchronous accessor calls in named computes qualify. Owners: `execution_role.rs`, `rust/crates/solid-reactive-ir/src/findings.rs`.

<a id="adr-0302"></a>
## ADR 0302 — Deferred apply needs a compute dependency
An effect apply's phase is not proof it runs: with literal `defer: true` (dialect `effect_apply_defer_options`) the apply is claimed only when the compute has a proven changing dependency (writable primitive accessor with a retained setter, read on every completion); proven absence suppresses it; unknown options claim nothing. Omitted options and `defer: false` keep initial execution. Owner: `rust/crates/solid-reactive-ir/src/effect_execution.rs`.

<a id="adr-0303"></a>
## ADR 0303 — The generated program is never written into the project
The dependency-source sibling tsconfig's bytes live in a scratch directory (`$TMPDIR/solid-checker-programs` or `SOLID_CHECKER_PROGRAM_DIR`) and are opened by the producer as an overlay at a logical path beside the project's tsconfig (`-project-config`); read-only projects work; gates run with `SOLID_CHECKER_DAEMON=0`. Owner: `write_for_process`, `logical_path` in `rust/crates/solid-facts-backend/src/dependency_sources.rs`.

<a id="adr-0309"></a>
## ADR 0309 — A reactive value's flow is one fact (removed)
Superseded by deletion on 2026-10-19. Only a private observational `value_flow` stage ever existed: it ran when `SOLID_CHECKER_VALUE_FLOW_AUDIT` or timings were requested, kept every seed's coverage open and moved no finding, and the production migration stayed held. The module, its `value-flow-shadow` timing stage, its `BuildTimings` field and the audit variable are gone; no script or package read any of them. A flow fact that certifies would be a new design, not this one revived.

<a id="adr-0310"></a>
## ADR 0310 — A source-edge refusal is a caller's recorded withholding (removed)
Superseded by deletion on 2026-10-19. `PackageBoundary` could hold exact occurrence sites where a caller withheld source-boundary analysis, projecting one SC9005 `source-boundary-withheld:<form>` finding. No production caller ever constructed one (only unit tests did), so the record type, `source_refusal.rs`, the `BuildError::SourceEdge` variant and the projection branches were deleted together with the rest of the dead package-contract surface; a caller that needs the behavior again designs it from the current boundary.

<a id="adr-0311"></a>
## ADR 0311 — A dependency-source check is a fresh one-shot check
An ordinary invocation whose effective dependency selection is non-empty (named list, `*`, or the implicit default) never uses the retained analysis daemon and opens fresh Type Facts programs; `daemon::eligible` requires an empty selection and `--serve` refuses a non-empty one. Base mode (no selection) is unchanged. Not claimed: in-run filesystem coherence, WASM, direct retained sessions. Owner: `eligible`, `serve` in `rust/crates/solid-facts-backend/src/daemon.rs`.


<a id="adr-0312"></a>
## ADR 0312 — Package contracts are removed
Ordinary analysis uses project and selected dependency source plus the dialect's reviewed runtime premises; it consumes no catalog, authored tier, bundled contract or receipt. The package boundary derives from `package_requirements`; behavior unavailable from source is an explicit uncertifiable SC9005, never a guessed violation. Removed CLI and ESLint settings refuse with migration messages; `--no-package-contracts` is a no-op; WASM `checkSync` keeps bare runtime package imports open. SC9005 is renamed `package-behavior-unavailable` (the former `package-contract-incomplete` stays an alias in `rule-options.json` and ESLint configs), and the JSON `coverageFamily` value `package-contract` becomes `package-boundary`. Owners: `rust/crates/solid-facts-backend/src/package_requirements.rs`, `packages/cli/eslint.cjs`.

<a id="adr-0313"></a>
## ADR 0313 — A string tag renders an intrinsic element
`Dynamic` with a string-literal `component` is an intrinsic element for ADR 0218's event-only proof: the dialect names the export and prop per release family (`intrinsic_tag_renderers`; Solid 2 `@solidjs/web` `Dynamic`/`component`); un-namespaced `on…` keys become listeners. The string must be proven per hop through attributes, `omit`/`merge` views and aliases; unknown spreads, getters and computed keys leave it unproven. Owners: `Dialect::intrinsic_tag_renderers`, `intrinsic_tag_prop` in `rust/crates/solid-reactive-ir/src/indexes.rs`.


<a id="adr-0314"></a>
## ADR 0314 — A prototype write reaches only its chain
A write through `R.prototype[k]` or `R.prototype = …` is narrowed to `R`'s instances when `R` is a never-rebound local class, function or `const` function/class expression, or the parameter of a static method whose every call passes an ambient `lib` host constructor; anything else keeps the project-wide veto. Built-in and fresh-literal origins are reached only by non-DOM host writes. Owners: `assigned_members`, `prototype_owner`, `member_value_symbols_in` in `rust/crates/solid-reactive-ir/src/indexes.rs`.
<a id="adr-0315"></a>
## ADR 0315 — A literal takes the role its callee runs it in
A function literal that is exactly an argument of an exact project callee, a data property of a closed options literal so passed, or a component prop takes the single role (event, deferred or tracked JSX) in which every call of it in the callee runs, when the callee is synchronous, its parameter a plain unwritten identifier, and every use a member read, test, call or another placed prop (`this`-exposing siblings must be arrows or absent under the census). Write legality never asks. Owner: `callee_use_site_role` in `rust/crates/solid-reactive-ir/src/execution_role.rs`.
<a id="adr-0317"></a>
## ADR 0317 — An imported base class is resolved
The demand plan requests the compiler symbol at every class's `extends` expression, so `class_instance_is_exact` follows an imported base across modules. Owners: `rust/crates/solid-facts-backend/src/demand_plan.rs`; `SemanticLookup::class_instance_is_exact` in `rust/crates/solid-reactive-ir/src/indexes.rs`.

<a id="adr-0318"></a>
## ADR 0318 — Script-free DOM members are trusted in the optional form
In the leaf-scope walk an optional call `el?.m()` is read like the non-optional form only when its exact `lib.dom` declaration is in an audited table of members that dispatch no event, run no custom-element reaction and change no connectedness: listener add/remove, `querySelector(All)`, `getElementsBy…`, `getBoundingClientRect`/`getClientRects`, `contains`, `closest`, `matches`, attribute reads, `toDataURL`. Every other optional DOM member refuses. Owner: `SCRIPT_FREE_DOM_MEMBERS`, `script_free_dom_member` in `rust/crates/solid-reactive-ir/src/cleanup.rs`.



<a id="adr-0319"></a>
## ADR 0319 — A callback takes the role of every invocation it reaches
In a closed program with no `eval`, a synchronous non-component literal takes the one role of every invocation found by a forward walk through exact holders: `const` bindings, object-literal members and spreads (static keys; no getter, `__proto__` or `this`/`super` use), returns, exact project callee arguments, component attributes, intrinsic `on…` attributes, conditional and logical operands. Roles must agree (event, deferred, tracked); mixes of non-reporting roles become `Unknown`; "nothing invokes it" counts only in a complete module census; scripts refuse. Answers are bound into `cross_file_proof_digest`. Owners: `invocation_flow_role` (`rust/crates/solid-reactive-ir/src/execution_role.rs`), `invocation_flow_answers` (`indexes.rs`).

<a id="adr-0320"></a>
## ADR 0320 — An unresolved alternative withholds the whole value
Per-call-site member dispatch trusts a value's implementations only when every part resolved (both conditional arms, every factory return, the binding initializer, the literal's last static-key own property with no later spread, getter, computed or `__proto__` key). A `const` holds its initializer only if unexported and every other reference is a member read or an argument of a closed callee. Unmodeled parameter uses, extracted or destructured members, defaults and forwards raise SC9012 unless the argument is a primitive proven from its own expression (never a type or assertion) or the site decides nothing (after an unconditional `await`). Owners: `member_value_candidates`, `proven_primitive` (`rust/crates/solid-reactive-ir/src/indexes.rs`), `attribution.rs`.
<a id="adr-0321"></a>
## ADR 0321 — An argument literal's function property is its implementation
An arrow or `function` expression that is exactly the value of a static-keyed property of an object literal written directly as a call's argument has identity `literal-property:<file>:<start>:<end>` and contributes no reads at the dispatch (its reads stay attributed where written); it resolves only when the callee only reads that parameter's members, and the name census is not consulted for it. Owners: `literal_property_function_symbol` in `rust/crates/solid-reactive-ir/src/interproc.rs`; `member_value_candidates` in `indexes.rs`.


<a id="adr-0322"></a>
## ADR 0322 — A static namespace member binds like a named import
A runtime namespace import whose every reference is a non-computed member object, a JSX member tag or a type-position use has each member redirected to the selected module's export as `import { m }` would be; its `NamespaceValue` obligation is dropped only if every namespace binding of the specifier qualifies and every member binds. Any value use refuses. `X.m()` is analyzed as a bare call. Owners: `namespace_static_members` in `rust/crates/solid-facts-backend/src/source_redirects.rs`; `namespace_import_redirected` in `package_requirements.rs`.


<a id="adr-0323"></a>
## ADR 0323 — `children` is a tracked lazy memo; a props or store read in a callback follows its consumer
`children(fn)` argument 0 is a tracked lazy `createMemo` (owner `Creates`, no during-call timing). A props or store read in an anonymous callback is classified like its signal twin by the role and timing machinery (ADR 0315/0319/0321); named helpers go through summaries. `createRoot`/`runWithOwner` callbacks inherit the calling window; `omit`/`merge` views are never proven sources and a spread of a view is never a violation. Owners: `rust/crates/solid-dialect/src/solid_2.rs`; `solid-reactive-ir/src/execution_role.rs`, `local_access.rs`.
<a id="adr-0324"></a>
## ADR 0324 — Route parameters are URL segments in the audited router
For an audited `@solidjs/router` release (Solid 2: `2.0.0-next.26`, from the compiler's attested import, installed bytes matching) the project does not replace (`paramsWrapper`, `queryWrapper`, `wrapParams`, `RouterIntegration`, `RouterContext`), `const params = useParams()` members and `<Show when={params.k}>` callback values are strings or `undefined`, feeding ADR 0320's primitive proofs. An unaudited resolved release raises SC9014. Owners: `router_params_status` in `rust/crates/solid-reactive-ir/src/router_params.rs`; `Dialect::router_params_are_url_strings`.
<a id="adr-0325"></a>
## ADR 0325 — An imported const holds its initializer's primitive
A named value import whose attested resolution lands on the declaring module (no barrel or `export *` hop), a module-level `const` exported under exactly the imported name (not CommonJS-touched), holds a primitive when its initializer passes `proven_primitive` in that file. Mutable exports, default imports, namespace members and package imports refuse. The set of such constants is in the cross-file digest. Owner: `imported_const_is_primitive` in `rust/crates/solid-reactive-ir/src/indexes.rs`.

<a id="adr-0326"></a>
## ADR 0326 — A component's function child is its `children` prop
A function literal that is the whole child of a non-intrinsic element is the component's `children` prop: its compiler `Deferred` role and any enclosing tracked region place nothing, so it keeps the lexical rendering role with unproven timing (uncertifiable) unless ADR 0319's walk places it. The body-call proof is not applied to children (ADR 0204's attribute proof only). Owner: `component_child_function` in `rust/crates/solid-reactive-ir/src/execution_role.rs`.

<a id="adr-0327"></a>
## ADR 0327 — A callback only an event runs has no owner
A plain function literal whose every invocation, found by ADR 0319's walk in owner mode (any primitive slot refuses; a call's role is event only when the function carrying it is a compiler event handler), is an event dispatch is seeded unowned, so an owner primitive directly in it is an SC4001 violation. Premise: an event handler runs with no owner (synchronous dispatch from an owned scope breaks it; recorded in the backlog). Owners: `unowned_event_literals` in `rust/crates/solid-reactive-ir/src/execution_role.rs` and `indexes.rs`; `owner_node` in `owners.rs`.


<a id="adr-0328"></a>
## ADR 0328 — Uncertifiable results are internal
Users see a finding when it is not uncertifiable or is a project-level notice (`subjectKind: "project"`: SC9013, SC9014). Human formats show only those and `--certify` fails on a violation or notice; `--format json` and WASM keep every finding and the certification status; `--internal` restores the ADR 0202 tiers and strict certification; ESLint reports only user-facing findings. Owners: `user_facing`, `certify_fails`, `render_user` in `rust/crates/solid-facts-backend/src/snapshot_emission.rs`; `userFacing` in `packages/cli/eslint.cjs`.


<a id="adr-0329"></a>
## ADR 0329 — A DOM call that dispatches synchronously is not trusted
In the leaf-scope walk a call is refused, optional or not, when its exact `lib.dom` declaration is `focus`, `blur`, `click`, `dispatchEvent`, `checkValidity` or `reportValidity` on any interface (and window globals), or `HTMLFormElement.reset`/`requestSubmit`, `HTMLElement.showPopover`/`hidePopover`/`togglePopover`, `HTMLDialogElement.show`/`showModal`/`close`/`requestClose`, `AbortController.abort`, `XMLHttpRequest.open`/`abort`, `Document.execCommand`. Tree mutation and attribute writes stay trusted. Owner: `synchronously_dispatching_dom_member` in `rust/crates/solid-reactive-ir/src/cleanup.rs`.


<a id="adr-0330"></a>
## ADR 0330 — A ref creation is SC6001 only in the unowned apply phase
A creation inside a `ref` expression is an SC6001 violation only when it runs when the applied callback runs and nothing between gives it an owner. Eager arguments, IIFEs and default parameters run while rendering (owned); a `createRoot` callback, directly or through inline synchronous slots, owns it. Any other owner-supplying slot (`runWithOwner`), a project helper, an unread callee, a discarded factory result or a generator makes it uncertifiable. Premise: a per-element `createRoot` is never disposed. Owners: `ref_phase`, `nested_owner`, `discover_directive_creations` in `rust/crates/solid-reactive-ir/src/directives.rs`.

<a id="adr-0331"></a>
## ADR 0331 — A function is a component only where `createComponent` calls it
A function returning JSX is a proven component when every reference to it is its declaration, module surface, a dialect callback, a dialect renderer prop (ADR 0138) or a proven `lazy` default (ADR 0292). Any other reference (alias, object storage, a call argument, a method, an anonymous default export, a router config not read from source) leaves it analyzed as a component, but its body is an `UndecidedRegion`: every non-style violation there is uncertifiable. Callbacks a dialect primitive places itself, and nested functions rendered by a tag, are carved out. Owners: `component_statuses`, `undecided_component_regions` in `rust/crates/solid-reactive-ir/src/indexes/components.rs`; `site_in_undecided_component` in `lib.rs`; the demotion in `rust/crates/solid-facts-backend/src/diagnostics.rs`.

<a id="adr-0332"></a>
## ADR 0332 — A site no fact places is an obligation
A read, write or action whose execution role is `Unknown` (no fact) is projected as an uncertifiable finding of its rule (SC1001, SC2001, SC2002), never dropped; one predicate per seed (`is_uncertifiable`) sets kind and wording. `Unreported` is the proven no-breach answer without a single role (ADR 0323's merge of non-reporting roles, code past the strict-read window): every consumer treats it as `Unknown`, projection reports nothing. The same holds for a read in a callee called after an `await` in a tracked async computation (SC1002, through the reads attributed to that call), `resolve`/`until` in code no role places (SC2004/SC2005), options the syntax cannot read beside an async computation (SC7002), and a dialect primitive referenced as a value (SC9012, `PRIMITIVE_VALUE`). A coverage rule (`RuleMetadata::uncertifiable`) disabled by options hides its findings but keeps the status from `certified`. Two identity rules guard the primitive lookup: a call of a method of a primitive (`createEffect.call`/`.bind`) is not a call of the primitive, and an import binding whose symbol declares nothing (an unresolved import, which shares the checker's one unknown symbol) names no primitive. Owners: `projection.rs`, `ReactiveRead`/`ReactiveWrite`/`ActionInvocation::is_uncertifiable`, `referenced_primitive` and `call_primitive_name` in `lib.rs`; `add_solid_import_names` in `symbols.rs`; `reactive_read_after_await_through_calls`, `primitive_value_references` in `static_rules.rs`; `resolve_scope_unplaced` in `static_api.rs`; `retain_enabled` in `rust/crates/solid-facts-backend/src/diagnostics.rs`.

<a id="adr-0333"></a>
## ADR 0333 — A write past a dominating await runs with no owner
A write is legal, and reports nothing, when it lies past an await that dominates it: the await is at statement level of its innermost block with nothing between the block's start and it that can skip it, and the write is later in that block, in the same async function, not a generator, and not in an instance field, JSX or a compiler tracked region (facts schema 65, `dominating_awaits`). A `try` block counts; its `catch` and `finally` count only when the block's first statement awaits an operand that cannot throw synchronously (a call of an identifier naming an async non-generator function declaration, with only literals and enclosing-function parameters as arguments: a hoisted declaration is initialized, calling an async function never throws, and its native promise is adopted without project code), facts schema 66 `try_head_awaits`. A read there is past the strict-read window and silent too. Premises: code after an await runs on a later task with no owner, and no program redefines `Promise.prototype.constructor` or `then` (which an await of a native promise reads synchronously; as ADR 0190 for primitive prototypes) (runtime-oracle `precision-write-after-try-await`, `precision-write-in-catch-after-async-call`). An event-only proof for writes was built and dropped: a synchronous dispatch from an owned scope runs the handler under that owner (review round 4). Owners: `follows_dominating_await`, `follows_try_head_await` in `rust/crates/solid-reactive-ir/src/execution_role/body_roles.rs`, `semantic_write_execution_role_within` in `write_roles.rs`, the visitor in `rust/crates/solid-facts/src/ast/visitor.rs`.

<a id="adr-0334"></a>
## ADR 0334 — An audited TanStack router renders its route components
A function referenced as the `component`, `pendingComponent`, `errorComponent` or `notFoundComponent` value of the options literal handed to an exact named import of `createFileRoute(path)(…)`, `createLazyFileRoute(id)(…)`, `createLazyRoute(id)(…)`, `createRootRouteWithContext()(…)`, `createRoute(…)` or `createRootRoute(…)` from `@tanstack/solid-router` is rendered there, which accounts for that reference in ADR 0331's census. The audited releases (`2.0.0-beta.29`, `rc.0`, `rc.4`, `rc.6`, each with the `@tanstack/router-core` it loads; all three builds read) only render these values or read a member such as `preload` of them, and never call one. As ADR 0324, the premise applies only when every runtime import of the router resolves through the attested resolution, agreeing with the bundler's, to an audited release whose bytes and whose router-core's bytes (resolved from the router's real directory) hash to the recorded digests, and no manifest, lockfile or patch directory records a replacement of either. It is withheld when any project file reads one of the four keys back (a member, a literal element key, a destructured key in a declaration or parameter): a value read back could be called plainly. Premise: the project does not reach route options by a key computed at run time or by reflection (`Object.values(route.options)`). `createRouter`'s `defaultComponent`, `defaultPendingComponent`, `defaultErrorComponent` and `defaultNotFoundComponent` count the same way (rendered at the same sites; the key-read veto covers them), and an anonymous inline function written at such a key is a component (`execution_role/callee_roles.rs` stops treating it as the call's callback). Owners: `rust/crates/solid-reactive-ir/src/route_components.rs`, `route_component_value` in `indexes.rs` read by `components.rs`; `route_install_roots`, `route_replacement_recorded` in `rust/crates/solid-facts-backend/src/installed_bytes.rs`; `AUDITED_ROUTE_RELEASES` in `rust/crates/solid-dialect/src/solid_2/releases.rs` (fixtures `route-components*`).

<a id="adr-0335"></a>
## ADR 0335 — An audited @solidjs/router renders its route definitions
A function referenced as the `component` of an object-literal route definition written in the `routes` array of `createRouter({ routes: [...] })`, an exact named import of `@solidjs/router`, or in such a definition's `children` array (recursively), is rendered there, which accounts for that reference in ADR 0331's census. The audited releases (`2.0.0-next.16`, `next.18`, `next.26`; `dist/` for both export conditions) copy the `component` into the route record, render it only with `createComponent` in the outlet, and otherwise read only its `preload`. As ADR 0324 and 0334: every runtime import resolves (attested, agreeing with the bundler) to an audited release whose bytes hash to the recorded digest, no record replaces the package, and no analyzed file outside the audited installs can obtain a definition. The router hands definitions out only through `useRouteMatches`, `RouterContext` and the instance's `routes`/`config` (route props, `preload` arguments and every other hook hold none), so the router may be imported only by name and never those two, never as a namespace, default, deep path, dynamic load or re-export, and every `createRouter` result must be a `const` whose every compiler reference is its declaration, module surface or a JSX tag, in a module no file reaches as a namespace (element access names no reference); any non-literal load, module hazard or `import.meta.glob` withholds the premise. The router calls `route.preload(...)` on the record and `config.transformUrl(...)` on the config, so the config and each definition count only with no getter or method and every key but `routes`, `component` and `children` a non-callable literal or an arrow. Premise (owner decision, as ADR 0190 and 0333): no program installs callables on `Object.prototype`, which the router would call with the config or a record as `this`; any `prototype`, `__proto__` or `setPrototypeOf` spelling and ADR 0292's reflective-prototype-write veto withhold the premise as a tripwire. A project `component` read then cannot be a definition's. Residual premise: no reflection over the router's module internals. Also followed: `defineRoute(...)`/`defineRoutes(...)` (identity helpers in the audited releases), a stock-history call at the config's `history` (ADR 0324), an anonymous inline component value, and a constant route table whose every compiler reference is its declaration, module surface or the `routes` of such a config, in a module no file reaches as a namespace. Not followed: definitions built by a project call, `children` thunks, `fileRoutes()` output. Owners: `router_config_values`, `router_directories`, `definitions_exposed` in `rust/crates/solid-reactive-ir/src/route_components.rs`; `AUDITED_ROUTE_CONFIG_DIGESTS` in `rust/crates/solid-dialect/src/solid_2/releases.rs` (fixtures `route-config*`).

<a id="adr-0336"></a>
## ADR 0336 — A shorthand property is a reference of its value
`{ value }` names the property it declares and reads the binding `value`. The Type Facts reference index skipped declaration names, so the read was in no reference list, and every census that requires all references of a binding to be accounted (ADR 0331's component identity, ADR 0335's router instance and route tables) missed an object holding the value: `const holder = { Chip }; holder.Chip()` was a false SC1001 (fixture `component-identity-escapes`, `Stored.tsx`). The index now records a shorthand's name under `GetShorthandAssignmentValueSymbol` as well. Owner: `scan` in `apps/solid-typefacts/internal/typefacts/tsgo/references.go` (`TestShorthandPropertyIsAReferenceOfItsValue`).


<a id="adr-0339"></a>
## ADR 0339 — A mention of a function-valued binding runs after its declarator only where that is proven
`const helper = () => …` is not hoisted: a mention that runs before the declarator throws `ReferenceError` (`var` reads `undefined`, `TypeError`) and the body never runs, and `tsc` is silent when the early use is inside a function body. A call, JSX render, primitive argument or callback role that names such a binding supplies a call site and a reachability edge only where the mention provably follows the declarator; otherwise it supplies none, so a finding resting on it is uncertifiable or absent, never a violation. In one module the mention follows it when it is in the declarator's own body after it (and the declarator is not directly in a `switch` case), inside the function itself, in a function expression or arrow created after it, or in a hoisted module-level declaration that nothing running ahead of the declarator can call: no function written at module level before it, and no declaration mentioned by such code or by module-level code before it, mentions that declaration, and the module is on no import cycle. Across modules it follows when the caller statically imports the declaring module and the declaring module is on no cycle (ES evaluation order); a module on, or importing, a cycle or with an unresolved relative import is not proven. Premises: ES module semantics (no CommonJS cycles), and module-level code before the declarator runs project code only through functions written there or declarations it mentions (getters and iterators are functions written there). A mention in a hoisted declaration nested in a function, or in a method, is not proven. Owner: `rust/crates/solid-reactive-ir/src/indexes/initialization.rs`; the withheld mentions are bound into the cross-file proof digest.

<a id="adr-0342"></a>
## ADR 0342 — A member of a compound component that only JSX member tags reach is a component
`export const Split = { Root, Panel }` and `Object.assign(TabsRoot, { List: TabsList })` store components in an object rendered as `<Split.Panel>`. A tag is `createComponent(Split.Panel, props)`, so when every way to obtain a member is a tag, `createComponent` is its only caller and the member is a component as ADR 0331's directly tagged function is; the stored reference no longer counts as an escape there. The holder qualifies when it is a `const` identifier bound to a closed object literal (no spread, computed key, duplicate, getter, setter or method; each value an identifier, an anonymous function, a data literal or a nested literal) or to the standard library's `Object.assign` (`mutation_builtin`, exact `Object`) whose target is a function and whose other arguments are such literals with no repeated key; when every Type Facts reference to its binding is its declaration or module surface (exports as ADR 0331 accepts them) or the leftmost name of a JSX tag, opening or closing, whose keys name a member of the tree and end at a value (`<Card.Header.Title>`; a plain `<Tabs>` only for an `Object.assign` target); when no reflective prototype write exists (ADR 0292); and, for an `Object.assign` target, when every reference to the target function is its declaration, surface, that argument or a plain tag (it is the holder's own value, so any other reference reaches every member). A value some tag renders is accounted in the census of the function it names; an anonymous function so stored is proven by that alone. A member nothing renders, a member read as a value, a plain call, a destructuring, a type-position mention of the holder (`typeof Split`), a namespace import of the holder's module used as `<ns.Split.Panel>` and a holder passed anywhere leave every member under ADR 0331. The member's body is then a component's, and a call written directly in it is no longer a retained-callable witness (`runs_in_unrendered_object_callable`, as ADR 0292's `lazy` default), so a read through a helper it calls stays a violation. Premise: as ADR 0331, the analyzed files are the whole program and nothing enumerates a module's exports reflectively. Not followed: `Object.assign(F, {...})` written as a statement or `F.List = ...` (the holder is the function), identifier-valued nested namespaces, and a holder reached through a namespace import. Owners: `indexes/compound.rs` (`compound_value`, `is_compound_render_witness`) read by `indexes/components.rs` and `interproc/retained_literal.rs` (fixtures `compound-components*`).
