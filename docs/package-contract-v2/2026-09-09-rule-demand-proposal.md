# Package contracts derived from rule demand

- **Status:** proposal; no implementation or acceptance-policy change authorized by this document.
- **Date:** 2026-09-09.
- **Decision proposed:** retain exact artifact authentication and the proof kernel; acquire and consume behavioral predicates demanded by rules instead of targeting closure of every claim domain of every export.
- **Scope:** ordinary Solid 1.x and Solid 2.0 rule analysis, its package-contract suppliers, and their measurement. This investigation changes only this document.
- **Product requirement clarified on 2026-09-09:** retain every current rule and make wrong usage of third-party package primitives a central supported use case. This proposal does not remove rules, change their presets, or restrict the product permanently to a small package portfolio.
- **Source basis:** the working tree, including existing modifications and untracked investigations, at HEAD `6b687543d3d0ef8109712e2601aec99776dd319b`. HEAD alone does not reproduce the source examined. Measurements below identify their own retained runs; they are not measurements of this working tree.

## Answer

The mission's premise is **partly right**. Four of the nine behavioral claim domains—`writes`, `invalidates`, `throws`, and `disposals`—have no behavioral consumer in the current rule path. Their active acquisition has no demonstrated finding value today. They are deletion candidates only after checking whether the missing consumer is needed to extend an existing rule to third-party primitives (§1.5). Whole-export closure is a poor product objective: no rule asks whether all nine domains are closed.

The stronger premise, that the pipeline must prove everything before anything is useful, is wrong. Certification already retains verified positive facts while sibling domains remain open. In the retained September 9 full run, accepted root catalogs contain 664 positive operation occurrences across four domains. A source-based check of today's projection finds 616 projectable occurrences; that is evidence of available inputs, **not 616 enabled findings or 616 soundly discharged proof obligations**. The remaining 48 are returns whose open object or tuple collections the consumer rejects.

The larger mismatch is between acquisition and consumption. Exact artifact selection is active and essential. The normalized model's call-site guard and possible/guaranteed-operation API has no production rule caller. Ordinary analysis projects accepted exports into a smaller representation, losing distinctions the rich model was built to preserve. Meanwhile, much of the closure effort establishes empty `creates` and empty `returns`, whose direct effect is often reducing an uncertifiable result or helping another package's census, rather than enabling a new defect finding.

Declarations are a cheaper source of **premises**, not a substitute for behavioral proof. They can establish exact signatures and parameter types. They cannot establish callback timing, owner propagation, reactive identity, absence of getters, or disposal. The recommended design uses declarations and Type Facts first to select and constrain a small proof, then escalates to exact implementation evidence for the behavior that remains unknown.

Four unused domains are a 44% reduction in domain count, or 9 to 5—not evidence of a tenfold engineering saving. An order-of-magnitude gap between all-export work and actual application demand is plausible, but no retained rule-demand workload measures it. This proposal does not claim that number.

The clarified product direction is **option 1 with the full current rule set and expanding package support**. A small set of packages can establish an implementation slice, but option 2's permanent support restriction is not the recommended destination. The implementation target includes both facts current consumers read and facts missing consumers need to make existing rules work on third-party primitives. Shrinking that target to today's incomplete projection would preserve an implementation limitation instead of fulfilling the product requirement.

## 1. Inventory method and terminology

The inventory covers all **44 documented catalog identities**: 18 in the [Solid 1.x catalog](../../rust/dialects/solid-v1/rules/src/rules.rs) and 26 in the [Solid 2.0 catalog](../../rust/dialects/solid-v2/rules/src/rules.rs). There are 16 shared concepts and 12 dialect-only concepts. Paired matrix rows explicitly link both identities; a shared code does not imply shared runtime vocabulary. `docs/rules/README.md` is an index, not an additional rule.

The dialect catalogs and their `lib.rs` implementations own identity, applicability, severity and wording. Much of the behavioral analysis lives in shared IR modules. Following only the dialect directory would miss the contract consumers. I traced from accepted import binding through `contracts.rs`, `source_discovery.rs`, `reactive_analysis.rs`, `local_access.rs`, `interproc.rs`, execution/owner analysis and final dialect projection.

Following [CONTEXT.md](../../CONTEXT.md), package contracts are one **fact domain**. The nine columns below are **claim domains** within that fact domain. An uncertifiable result is a finding kind, not evidence of a violation. A verified possible operation does not imply a guaranteed operation; an open collection does not imply that its known items are exhaustive.

### 1.1 The actual field-level consumer interface

The following keys name consumption paths used in the matrices. Field names on `ExportSemantics`/`Operation` describe the normalized model; compact wire spellings are included where useful. These are source observations, not a proposed API.

| Key | Accepted fields actually read | Projection and downstream consumption | What absence does today |
| --- | --- | --- | --- |
| **B: binding and shape** | Exact package/version/integrity, importer and specifier, resolved export and declaration, artifact case and closure, receipt; `export.shape` | `AcceptedContractIndex` resolution → `contracts.rs::resolve_contract_imports_inner` / `project_accepted_export`. Callable/component becomes function; proven noncallable shape makes call-path effects vacuously empty. Unknown or possibly callable choice must stay open. | A recorded missing/unaccepted/mismatched use produces SC9005. A completely unknown package is not automatically diagnosed at every import; its particular call can produce SC9011 or another obligation. |
| **C: callback behavior** | `callbacks.items` and closure; `CallbackBinding.from = Parameter { index, path }`; referenced operation's `tracking`, `schedule` (wire `at.schedule`), `inputs`, `owner.source`, owner requirements/capabilities | `contracts.rs::project_callbacks` → `source_discovery` callback and callback-argument maps → `interproc` invocation edges; `execution_role::allowed_callback_spans`; `owners::owner_callback_edges`. Tracked → tracked, with schedule retained separately, possibly unestablished; otherwise same-stack → inline, queued/external → deferred. The parameter index survives but its path does not. Callback input reactive shapes can identify accessors passed into literal callbacks. Owner projection distinguishes none/created/leaf/inherited, but collapses captured and ambient owner sources into inherited. | An unprojectable callback leaves callback knowledge open; potentially callable arguments can then create SC9005. Unknown callback timing also prevents some nested reads/writes from becoming proven violations. Unbound callback arguments create a call-site obligation. Partial rows coexist with open knowledge; see the precision caveat below. |
| **D: direct and parameter reads** | `claims.reads.items` and closure; referenced operation's first `inputs` entry: `Parameter { index, path }`, `Reactive`, or `Store` | `contracts.rs::project_reactive_reads` → `source_discovery` → `local_access` direct/parameter read handling and `interproc` read summaries. The compact record retains a parameter path, but the downstream parameter-read tuple drops it and resolves the actual argument as a whole. | Open reads add SC9005 at a bound import. An unresolved actual argument for a described parameter read produces SC9012. With no source fact, the external-dependent read finding is absent; that is not a certificate of non-reactivity. |
| **T: returned reactive identity** | `claims.returns.items` and closure; referenced operation's `output`: `Reactive`, `Store`, `Parameter { index, path }`, recursively supported tuple/object members; tuple/object collection completeness | `contracts.rs::project_return` / `project_return_shape` → `source_discovery` accessor/store maps and `interproc` returned-value propagation. One distinct projectable return shape survives. Parameter returns preserve the index but lose the path. Reactive role/capabilities/resource identity are discarded. Open object/tuple collections are rejected wholesale. | Unknown, ambiguous or unprojectable return stays open and produces SC9005 for a bound import. A missing accessor/store identity prevents source-dependent findings. An unresolved described structured access may also yield SC9012. |
| **A: async protocol** | The **returns** domain, specifically operation `output = Promise` or `AsyncIterable` | `contracts.rs::project_async_behavior` → `owners::computation_is_async_with_contracts` → native computation discovery and `static_api` synchronous-computation check. It recognizes an imported computation callback, or a local callback returning a described imported call. | Open returns contributes SC9005 through returns. The async projection itself is `Known("")` when no recognized output exists; that must not be interpreted as proof of synchronous completion. Type Facts or explicit local async syntax can independently establish async behavior. This path does not establish that an arbitrary third-party returned accessor suspends. |
| **O: caller owner requirement** | **Creates and cleanups** operation IDs; `operation.owner.requirements.owner = Required`, `owner.source != Created`; operation kind `Cleanup`/`Dispose` versus other kinds | `contracts.rs::project_owner_requirements` → `owners` call-site owner requirements → SC4001. A callee-created owner is not a caller requirement. Missing-owner analysis also uses C for callback owner edges. | Open creates contributes SC9005. An open cleanup collection does **not** itself get inserted into `open_claims` here. Verified cleanup requirements survive partial knowledge. Missing requirements cannot justify a positive owner-safety conclusion. |
| **N: negative creates for composition** | `claims.creates.is_closed()` **and** empty items, exposed as `creates_closed_empty` | Read separately from the owner projection, for dependency composition / the proposal creates walk. It is not a new ordinary-rule behavioral predicate. | A caller's proposed empty creates cannot use an open dependency as proof; its closure stays withheld or uncertifiable. This is a real indirect proof dependency, so deleting all creates evidence would be incorrect. |

The ownership code's match on operation kind `Dispose` is **not** a read of the `disposals` claim domain: the iterator is over creates/cleanups. A similarly misleading reference occurs in `push_unknown_contract_claims`: it checks `open_claims.contains(Throws)` while reporting `asyncBehavior`, but `project_async_behavior` reads returns, and ordinary projection does not insert Throws. This is vestigial plumbing, not a throws-dependent rule.

Source owners:

- [Accepted selection and instantiation API](../../rust/crates/solid-reactive-ir/src/contract_semantics/consumer.rs), [normalization and model](../../rust/crates/solid-reactive-ir/src/contract_semantics.rs), [compact semantic projection and missing-claim findings](../../rust/crates/solid-reactive-ir/src/contracts.rs).
- [Source discovery](../../rust/crates/solid-reactive-ir/src/source_discovery.rs), [analysis wiring](../../rust/crates/solid-reactive-ir/src/reactive_analysis.rs), [direct access analysis](../../rust/crates/solid-reactive-ir/src/local_access.rs), [interprocedural propagation](../../rust/crates/solid-reactive-ir/src/interproc.rs).
- [Execution roles](../../rust/crates/solid-reactive-ir/src/execution_role.rs), [owner analysis](../../rust/crates/solid-reactive-ir/src/owners.rs), [static rules](../../rust/crates/solid-reactive-ir/src/static_rules.rs), [static API checks](../../rust/crates/solid-reactive-ir/src/static_api.rs).
- [Shared upstream reactivity checks](../../rust/crates/solid-reactive-ir/src/upstream_compat/shared_reactivity.rs), [structural preferences](../../rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_structure.rs), [leaf and cleanup classification](../../rust/crates/solid-reactive-ir/src/cleanup.rs), [directive analysis](../../rust/crates/solid-reactive-ir/src/directives.rs), [server analysis](../../rust/crates/solid-reactive-ir/src/server_rules.rs).

### 1.2 Rule × claim-domain demand matrix

**R = required for the identified external-dependent branch**, not required for every finding of that rule. D and T can be alternative ways to establish a reactive read. **I = improves source, context or helper propagation**, without directly proving the rule's principal defect. **U = unused by that rule's behavioral proof**. B is a prerequisite to every accepted external fact and is outside the nine behavioral columns. No R/I grants permission to use a possible operation as a guaranteed occurrence.

| Rule identity / identities | callbacks | reads | writes | creates | invalidates | throws | returns | cleanups | disposals | Actual path |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [strict-read-untracked](../rules/strict-read-untracked.md), [v1/strict-read-untracked](../rules/v1/strict-read-untracked.md) | R | R | U | U | U | U | R | U | U | C/D/T → read and execution analysis |
| [reactive-read-after-await](../rules/reactive-read-after-await.md), [v1/reactive-read-after-await](../rules/v1/reactive-read-after-await.md) | I | U | U | U | U | U | R | U | U | T/C source identity → `static_rules`; native tracked computation context |
| [no-destructure](../rules/no-destructure.md), [v1/no-destructure](../rules/v1/no-destructure.md) | I | U | U | U | U | U | R | U | U | T store identity, C allowed callback spans → `static_rules` |
| [components-return-once](../rules/components-return-once.md), [v1/components-return-once](../rules/v1/components-return-once.md) | I | R | U | U | U | U | R | U | U | D/T and C helper read summaries → conditional-return check |
| [uncalled-accessor](../rules/uncalled-accessor.md), [v1/uncalled-accessor](../rules/v1/uncalled-accessor.md) | I | U | U | U | U | U | R | U | U | T/C accessor map → shared reactivity |
| [reactive-handler-frozen](../rules/reactive-handler-frozen.md), [v1/reactive-handler-frozen](../rules/v1/reactive-handler-frozen.md) | I | U | U | U | U | U | R | U | U | T/C source identity → shared reactivity |
| [v1/reactive-write-in-owned-scope](../rules/v1/reactive-write-in-owned-scope.md) | I | U | U | U | U | U | U | U | U | C execution role; setter remains native/local |
| [reactive-write-in-owned-scope](../rules/reactive-write-in-owned-scope.md) | I | U | U | U | U | U | I | U | U | C execution role; T can identify native refresh/affects source targets |
| [no-direct-mutation](../rules/no-direct-mutation.md), [v1/no-direct-mutation](../rules/v1/no-direct-mutation.md) | I | U | U | U | U | U | R | U | U | T/C store/source identity → shared reactivity |
| [missing-owner](../rules/missing-owner.md), [v1/missing-owner](../rules/v1/missing-owner.md) | I | U | U | R | U | U | U | R | U | O plus C owner graph |
| [missing-effect-function](../rules/missing-effect-function.md), [v1/missing-effect-function](../rules/v1/missing-effect-function.md) | U | U | U | U | U | U | U | U | U | Native API + Type Facts/runtime argument classification |
| [jsx-no-duplicate-props](../rules/jsx-no-duplicate-props.md), [v1/jsx-no-duplicate-props](../rules/v1/jsx-no-duplicate-props.md) | U | U | U | U | U | U | U | U | U | JSX attribute/spread analysis |
| [prefer-for](../rules/prefer-for.md), [v1/prefer-for](../rules/v1/prefer-for.md) | I | R | U | U | U | U | R | U | U | D/T/C reactive read index + exact array map |
| [prefer-show](../rules/prefer-show.md), [v1/prefer-show](../rules/v1/prefer-show.md) | I | R | U | U | U | U | R | U | U | D/T/C reactive condition + JSX structure |
| [package-contract-incomplete](../rules/package-contract-incomplete.md), [v1/package-contract-incomplete](../rules/v1/package-contract-incomplete.md) | R | R | U | R | U | U | R | U | U | B, open C/D/T/A/O; describes missing proof, not a defect |
| [reactive-source-uncaptured](../rules/reactive-source-uncaptured.md), [v1/reactive-source-uncaptured](../rules/v1/reactive-source-uncaptured.md) | I | U | U | U | U | U | I | U | U | T/C identify handed source; **B membership alone** suppresses this obligation |
| [reactive-dispatch-unresolved](../rules/reactive-dispatch-unresolved.md), [v1/reactive-dispatch-unresolved](../rules/v1/reactive-dispatch-unresolved.md) | I | R | U | U | U | U | I | U | U | D unresolved actual parameter; T/C source/binding propagation |
| [v1/jsx-no-undef](../rules/v1/jsx-no-undef.md) | U | U | U | U | U | U | U | U | U | Scope resolution of JSX/namespaced uses |
| [v1/prefer-classlist](../rules/v1/prefer-classlist.md) | U | U | U | U | U | U | U | U | U | Configured structural preference |
| [action-called-in-owned-scope](../rules/action-called-in-owned-scope.md) | I | U | U | U | U | U | U | U | U | C execution role; native/local action identity |
| [resolve-in-tracked-scope](../rules/resolve-in-tracked-scope.md) | I | U | U | U | U | U | U | U | U | C execution role; native resolve identity |
| [leaf-owner-forbidden-call](../rules/leaf-owner-forbidden-call.md) | I | U | U | U | U | U | I | U | U | C owner context; T/C accessor identity enters helper safe-call set |
| [pending-async-unsuspendable-read](../rules/pending-async-unsuspendable-read.md) | I | U | U | U | U | U | R | U | U | A on native computation + C execution/owner context |
| [async-outside-loading-boundary](../rules/async-outside-loading-boundary.md) | I | U | U | U | U | U | R | U | U | A on native computation + C execution/owner context |
| [primitive-in-directive-application](../rules/primitive-in-directive-application.md) | U | U | U | U | U | U | U | U | U | Compiler directive roles + native/local helper primitive analysis |
| [sync-computation-received-async](../rules/sync-computation-received-async.md) | U | U | U | U | U | U | R | U | U | A → `computation_is_async_with_contracts` / `static_api` |
| [http-response-after-flush](../rules/http-response-after-flush.md) | I | U | U | U | U | U | U | U | U | C deferred spans; own native response/render model |
| [server-function-module-directive](../rules/server-function-module-directive.md) | U | U | U | U | U | U | U | U | U | Module/compiler classification |
| [server-function-rich-argument](../rules/server-function-rich-argument.md) | U | U | U | U | U | U | U | U | U | Type Facts argument classification + dialect transport model |

SC7003/SC9003 refresh/affects target checks mentioned by the Solid 2.0 catalog are additional target-validity branches in `static_api`, not additional documented catalog rows. They use exact native API identity and accessor/store classification, including T. They do not consume `invalidates`, `writes`, or `throws`. Contract generation obligations also project to SC9005; they must not be counted as new behavioral defect families.

### 1.3 What each rule proves, absence, and fidelity tiers

These are **incremental yields over the existing built-in runtime model, compiler execution facts, Type Facts and local analysis**. Tier (a) removes third-party behavioral contracts, not those other fact domains. Tier (b) adds authenticated declarations and Type Facts but no implementation evidence. Tier (c) provides the exact demanded behavior through today's accepted-contract path. “Same” means no additional behavioral capability from that tier, not zero findings overall.

The saved corpus does not contain per-rule contract-ablation runs on consumer applications. Consequently, the last column states the additional branch that can change a verdict, not an invented count of real-world violations. Matrix pairs share the branch described below; dialect-specific differences remain governed by their owners.

| Rule / code | Claim the rule attempts to establish | (a) No behavioral fact / today's absence | (b) Declarations only: additional yield | (c) Proven demanded behavior: additional yield |
| --- | --- | --- | --- | --- |
| strict-read-untracked / SC1001 | A reactive read executes where the selected dialect will not track or refresh it. | Native/local reads remain; an unknown external read/source is not proven. Bound open behavior yields SC9005; unknown callback timing blocks some nested violations. | Can resolve calls and value-only arguments; cannot label a function reactive or say when it invokes a callback. | D/T identify external reads/sources; C can place local callback reads in an execution context or propagate them through inline helpers. |
| reactive-read-after-await / SC1002 | A reactive read occurs after tracking has ended at an await in a recognized tracked computation. | External source identity missing → that read is not proven; local/native reads remain. | Await dominance and async function facts already help independently; a callable declaration adds no reactive identity. | T and reactive callback-input shapes extend the source set. An arbitrary contracted tracked callback does not become a native tracked async computation in `tracked_async_computation_context`. |
| no-destructure / SC1003 | A reactive props/store value is captured by destructuring in a context where updates will be lost. | Props/native stores remain; unknown external store is missed or accompanied by SC9005. | Object shape does not establish a reactive proxy. | T identifies returned stores; C can alter the execution-context exemption. |
| components-return-once / SC1004 | A component's reactive conditional return is evaluated once rather than becoming reactive control flow. | Native/local reactive conditions remain; unresolved external reads cannot establish the premise. | Types can resolve the condition expression but cannot prove reactive reads. | D/T and helper callback/read propagation reveal external-dependent reactive conditions. |
| uncalled-accessor / SC1005 | A known reactive accessor is used in a value position without being called. | Unknown external accessor is not diagnosed as one. | `() => T` is only callability; even a familiar alias spelling is not runtime reactivity. | T or a described callback input supplies accessor identity. |
| reactive-handler-frozen / SC1007 | A handler position receives a proven non-handler or freezes a reactive binding that will not rebind when its source changes. | Native/local source and runtime handler-shape cases remain; unknown external reactive value is not proven. | Callable handler/property types do not establish reactive identity or listener timing. | T/C source discovery expands reactive handler cases; it does not describe a third-party event framework. |
| reactive-write-in-owned-scope / SC2001 | A recognized write executes in a dialect-owned reactive execution scope where it is forbidden. | Native setter/write cases remain; unknown callback context may make the case uncertifiable. | Can resolve symbols and admissible argument forms; cannot prove callback timing or package writes. | C changes execution classification; v2 T can identify sources used by native refresh/affects. No third-party setter or write operation is learned from the writes domain. |
| no-direct-mutation / SC2003 | Code writes directly through a known readonly reactive source/proxy. | Unknown external proxy is not proven readonly/reactive. | A readonly typing error is TypeScript's job; declared mutability does not prove proxy behavior. | T/C add reactive source identity for otherwise type-valid writes. |
| missing-owner / SC4001 | An owner-requiring operation executes without an owner that can retain and dispose it. | Native/local ownership remains; missing external requirement cannot prove a leak. Bound open creates yields SC9005; absent cleanup closure alone does not. | A cleanup-shaped return or callback type does not prove registration, ownership or disposal. | Creates/cleanups O supplies caller requirements; C can establish a callback owner edge. Local returned-cleanup classification is separate from the unused disposals domain. |
| missing-effect-function / SC7001 | A required computation/apply function is absent or definitely noncallable in the selected dialect's effect form even though the published typing did not prevent it. | No package behavior needed; unresolved runtime shape remains uncertifiable within this rule. | Type Facts already constrain callable/invalid/unknown cases; duplicate TypeScript diagnostics remain excluded. | Same. |
| jsx-no-duplicate-props / SC8003 | An intrinsic element has competing DOM content sources, or in v1 differently spelled props that lower to the same single-winner compiler slot. | No package behavior needed. | Existing syntax/type/compiler facts suffice for the supported cases; identical-name duplication and the isolated children-prop/JSX-children conflict remain TypeScript's. | Same. |
| prefer-for / SC8014 | A reactive array map in JSX is eligible for the configured keyed-control-flow preference. | Local/native reactive receivers remain; unknown external dependency is not established. | Array/map identity can be resolved, but reactivity cannot. | D/T/C add reactive receiver/dependency evidence. This is a preference, not proof that every array map is a runtime defect. |
| prefer-show / SC8015 | A reactive JSX conditional is eligible for the configured Show preference. | Local/native conditions remain. | Condition types do not prove reactivity. | D/T/C add external-dependent reactive conditions. |
| package-contract-incomplete / SC9005 | An external import/call lacks an accepted binding or behavioral fact needed by the current analysis/projection. | Missing or recorded rejected binding, bound open claims, or unbound callable arguments produce an uncertifiable result; not every unknown import is reported. | Exact declarations can settle identity, shape and binding premises; behavioral open domains remain open. | Satisfying relevant fields can remove or narrow SC9005. Other open domains can keep an import-level finding even after a particular behavioral branch becomes usable. |
| reactive-source-uncaptured / SC9011 | A known reactive source is handed to a resolved external callable whose capture behavior is undescribed. | Produces this uncertifiable result when the source and external callable are established; definitely value-only parameters can avoid it. | Exact parameter runtime value domain helps reject non-callback interpretations; does not prove timing/capture. | Today any `contracted.contains_key(callee_symbol)` suppresses SC9011. An all-open accepted binding can therefore move uncertainty to SC9005 without establishing safety. This must not count as finding enablement. |
| reactive-dispatch-unresolved / SC9012 | A potentially reactive dispatch/binding cannot be resolved precisely enough to classify its read. | Existing unknown dispatch remains uncertifiable. | Exact symbols/signatures can settle some dispatch without behavior; unresolved member targets stay unresolved. | D creates a concrete parameter-read obligation; T/C improve source propagation. It cannot resolve arbitrary third-party methods by resemblance. |
| v1/jsx-no-undef / SC8005 | A checker-owned JSX binding use, including namespaced directive uses not bound by TypeScript, has no declaration in scope. | No package behavior needed. | Existing binding facts suffice; the same TypeScript error must never be duplicated. | Same. |
| v1/prefer-classlist / SC8013 | A configured class-composition form can use the dialect's classList preference. | No package behavior needed. | Same. Configured preference matching is not behavioral package trust. | Same. |
| action-called-in-owned-scope / SC2002 | A recognized action is invoked in a forbidden reactive execution scope. | Native/local action cases remain; unknown callback timing can block proof. | Types cannot establish action runtime identity. | C changes context. `ValueShape::Action` is not projected as a returned source, so proving third-party action returns does not currently enable this rule. |
| resolve-in-tracked-scope / SC2004 | Native resolve is called with an active tracked execution context where it takes an invalid one-shot snapshot. | Native/local cases remain. | No extra callback execution proof. | C changes context. Neither invalidation behavior nor thrown errors are queried. |
| leaf-owner-forbidden-call / SC3001 | A recognized leaf owner runs an operation its ownership restrictions forbid. | Native/local helper analysis remains; unknown helper behavior stays unresolved. | Types do not prove owner creation or cleanup registration. | C can change owner context; T/C accessor identity enters `safe_call_symbols`. External operation graphs/creates/disposals are not used to enumerate forbidden helper effects here. |
| pending-async-unsuspendable-read / SC5001 | A known async source is read in a context that cannot suspend it under the dialect's execution/owner rules. | Type Facts and native computation cases remain; unknown async provenance is not established. | A selected computation callback's async return fact can help already; it does not prove an arbitrary returned accessor suspends. | A can classify a native computation whose imported callback or returned call is asynchronous; C improves context. |
| async-outside-loading-boundary / SC5003 | A known async source reaches rendering without the required Loading ownership boundary. | Native/local async cases remain. | Same limit as SC5001. | A extends native async provenance and C context. Boundary and compiler execution evidence remain necessary. |
| primitive-in-directive-application / SC6001 | A compiler-proven directive application invokes an owner-attaching native primitive in its ownerless application phase. | Native/local/directive helper cases remain; unknown external helper is not proven by this rule. | Can resolve declarations, not directive owner behavior. | No direct behavioral gain here. An external O requirement can separately enable SC4001 at the call. |
| sync-computation-received-async / SC7002 | A synchronous native computation receives a runtime computation that can complete asynchronously. | Local async syntax and Type Facts remain; unknown imported runtime completion is not proven. | Async function facts can independently establish the premise where typings expose it; do not emit the same claim as a TypeScript error. | A can provide missing runtime protocol evidence. It does not require throws closure. |
| http-response-after-flush / SC7005 | A response mutation may occur after streaming has flushed the shell. | An uncertifiable timing hazard based on native/render facts remains. | No package execution timing from declarations. | C can exclude deferred callback bodies from an immediate render scope. This is not proof that a particular request has already flushed. |
| server-function-module-directive / SC7006 | A server-function export is in a module form incompatible with the assumed compiler transform. | No external behavioral contract required. | Module/export facts help; transform authority must come from the compiler/build contract. | Same; package claims cannot repair a missing transform premise. |
| server-function-rich-argument / SC7007 | A server-function call carries a value the selected transport model cannot encode. | Native transport/type-shape analysis remains. | Argument Type Facts are useful already; serialization policy is not inferred from a declaration's parameter name/type. | Same; the rule never asks a package throws domain. |

The TypeScript boundary applies to every row, not just the rows mentioning it. Any implementation of this proposal must demonstrate type-valid positive cases against the real published declarations. An invalid callback return must not become a new contract-powered diagnostic merely because a fixture stub accepts it.

### 1.4 Limits discovered at the consumption boundary

These limits matter before treating “more accepted facts” as “more sound findings”:

1. `AcceptedContractUse::instantiate`, `AcceptedContract::instantiate_export`, `possible_operations` and `guaranteed_operations` are implemented and tested. Repository call-site search finds no ordinary-rule invocation of that API; ordinary rules go through `project_accepted_export` instead. Tests of instantiation establish model behavior, not rule adoption.
2. The compact projection does not carry operation cardinality, guard selection, trigger identity or operation-graph edges. A partial callback list is later tested for “exclusively deferred” by checking its known rows. The unknown-callback fallback is an `else if`. Exhaustiveness of known rows is not established by that test. This is a source-level proof gap to exercise with a focused fixture, not a newly reproduced false finding in this investigation.
3. Callback-origin paths, read parameter paths and parameter-return paths are lost downstream; reactive output role/capabilities/resource identity are discarded. Recognizing an accessor is not automatically a proof that calling it is harmless to a leaf owner. A replacement must preserve the predicate's subject and the capability required by its consuming rule.
4. T rejects all open tuple/object return collections, even when one member has positive information. A sound member-level query might use that information without closing unrelated siblings, but only after proving the selected member's value relationship and relevant guard/cardinality. Accepting all positive members indiscriminately would be unsound.
5. `external_packages` excludes core packages from ordinary accepted-contract use. Both [Solid 1.x](../../pkg/contracts/bundled/solid-v1/bundle-index.json) and [Solid 2.0](../../pkg/contracts/bundled/solid-v2/bundle-index.json) bundle indexes currently have empty `contracts` arrays. Ordinary core behavior comes from the explicitly selected **built-in runtime model**. Historical bundled core documents and conformance models are not evidence that ordinary rules consume all their behavioral domains.

The existing [September 9 rule soundness memo](audits/2026-09-09-solid2-rule-soundness-memo.md) independently records several projection/authority concerns. Its findings are source-review leads, not executed counterexamples. This proposal does not certify or repair those rules. In particular, “fully proven from the artifact as today” describes acquisition fidelity; it does not demonstrate that every downstream projection preserves that fidelity.

### 1.5 Required third-party capability beyond today's consumer inventory

The matrix above is descriptive. A U cell means no current behavioral consumer; it does not mean that third-party misuse covered by that rule is outside the product. The next design step must establish the smallest sound external predicate for each applicable existing rule before finalizing domain deletion.

| Existing rule capability | Required behavior across a package boundary | Consequence for the design |
| --- | --- | --- |
| Accessor misuse, frozen reads, store destructuring and direct mutation | Exact reactive identity and relevant capabilities of a returned value or selected member, propagated through local wrappers | Complete T/D consumption, preserving paths and all relevant return alternatives. A callable or object declaration alone is insufficient. |
| Owned-scope writes and action calls | Identity/capabilities of a returned setter or action, or proof that an invoked package helper performs the relevant write/action in the prohibited execution context | Today's accessor/store-only return projection cannot supply this generally. Add a consumer for the exact required capability or operation; retain writes acquisition if that is the necessary representation. Do not delete it solely because SC2001 currently recognizes mainly native/local writes. |
| Missing-owner and leaf-owner restrictions | A package operation requires a caller owner or performs a forbidden owner/cleanup operation within the relevant leaf scope | Existing caller requirements cover part of this. Extend exact helper-effect consumption where necessary. An unknown external helper is not safe merely because it returns an accessor. |
| Callback tracking, after-await reads and async execution | Exact callback argument, tracking/owner context, invocation schedule and relevant async behavior, with certainty appropriate to the rule | Connect the existing rule to the applicable external execution model; a single positive callback row does not establish exhaustive timing. |
| Compiler-, JSX- and server-specific rules | The particular compiler/transport premise the existing rule needs, where a package can affect it | Keep every rule. Package behavior contributes only when relevant; contracts cannot invent compiler authority or replace Type Facts. |

Each implementation slice must include a third-party misuse that produces the appropriate existing rule finding, a valid counterpart that remains clean, and a missing/mismatched-fact counterpart that is explicitly uncertifiable. Positive cases must type-check against the real published declarations. Bind all accepted behavior to exact package artifacts and the selected dialect. Do not promise a violation for every unsupported package: missing evidence must remain an analysis gap, never an invented defect or certified safety.

For the developer, the intended flow is automatic artifact selection and reuse of accepted behavior, followed by diagnostics at the misuse with a causal explanation. Manual proof-transcript authoring is maintainer tooling. Grouping analysis gaps is a presentation choice that must preserve their explicit uncertifiable status; it does not justify disabling any existing rule or silently ignoring the package boundary.

## 2. Cost and value from retained artifacts

### 2.1 Three different denominators

| Evidence | Denominator / what was counted | Observed result | What it cannot establish |
| --- | --- | --- | --- |
| [Accuracy roadmap](accuracy-roadmap.md), September 6 | Generator proposals: 8,950 export occurrences; 418 probes, 398 attempts | Creates closed 244, returns closed 44; each other domain closed 0; exports with all nine closed 0 | Accepted positive facts, actual consumer demand, or per-rule finding yield |
| [September 9 retained full run measurement](phase21/2026-09-09-async-binding-full-measurement.json), recounted for this investigation | Accepted **root** catalogs: 12,492 export-name × artifact-case × probe occurrences; 1,550 cases; 391 certified rows | 100 empty creates closures; 2 empty returns closures; other domain closures 0; all-nine-closed 0 | A longitudinal closure regression against the different proposal denominator; unique package/export count; dependency-catalog coverage |
| Same run's certification audits | All 409 attempted rows' recorded withheld closure candidates, including dependency nodes and retries | 3,102 recorded withheld candidates | A denominator directly comparable to root exports, or a count of distinct implementation defects |

The roadmap's printed “80 316 open” is internally inconsistent with its displayed table: `8,950 × 9 − 244 − 44 = 80,262`, a difference of 54. I did not silently replace its historical number or infer which input changed. Neither total is a useful rule-coverage metric.

The September 9 report finished at `2026-09-09T01:47:52.964Z` in **534.042 seconds**. It records 418 rows: **327 complete-entrypoint certifications, 64 partial-entrypoint certifications, 18 refused, 9 not advanced**. Its accepted selections contain 1,186 entrypoint occurrences (822 Solid 1.x, 364 Solid 2.0). “Complete entrypoint” does not mean complete behavioral knowledge.

I followed each report row's retained output directory, read its `certification-audit.json`, and followed the selected accepted catalog pointer rather than globbing abandoned publications. The 391 certified audits record ordinary-analysis receipt authentication and exact-case selection. I checked the hash-bound publication documents and receipts during counting. This was **not independent receipt replay or recertification**; the audit objects explicitly say they are non-authoritative and non-replayable.

### 2.2 Accepted facts available to the rule path

Counts below are export/case/probe occurrences, so reexports and floor/head probes can repeat a semantic implementation. Positive export counts overlap between domains.

| Claim domain | Exports with positive items | Positive operation occurrences | Closed domains | What today's projection can retain |
| --- | ---: | ---: | ---: | --- |
| callbacks | 124 | 145 | 0 | 145 timing/tracking rows; no owner source present in these operations |
| reads | 141 | 152 | 0 | 148 parameter-input reads, 4 reactive-input reads |
| returns | 304 | 304 | 2 | 158 reactive accessor shapes, 5 stores, 93 parameter identities; 48 open composite shapes rejected |
| cleanups | 63 | 63 | 0 | 63 caller owner requirements |
| creates | 0 | 0 | 100 | Empty-domain knowledge; owner-requirement completeness / dependency census input |
| writes | 0 | 0 | 0 | No rule consumer |
| invalidates | 0 | 0 | 0 | No rule consumer |
| throws | 0 | 0 | 0 | No rule consumer |
| disposals | 0 | 0 | 0 | No rule consumer |

There are **574 of 12,492 export occurrences with at least one positive item**, spread across **88 of 391 certified rows**. The source-based projection count is **616 occurrences across 82 rows**: 145 callbacks + 152 reads + 256 returns + 63 owner requirements. The difference is 25 open tuple returns and 23 open object returns. For example, retained `@kobalte/core@0.13.13` `createSize` and `@solid-primitives/broadcast-channel@0.1.1` `makeBroadcastChannel` include open object return information; the latter also has a separately usable cleanup owner requirement.

The callbacks split into **70 deferred/untracked, 58 same-stack/untracked, and 17 tracked** operations; the tracked operations split into nine same-stack and eight queued. All callback origins here are direct parameters with empty paths. None establishes callback owner propagation. There are **zero Promise/AsyncIterable return outputs** in these root catalogs, so the A path has no measured contribution from this retained set despite being a real source consumer.

All **664 operation occurrences have operation cardinality `min: 0, max: many, scope: call`**. They are not guarantees that a particular application call performs the operation. The 616 projection count is therefore an availability ceiling under the current representation, not a finding-enablement count. In particular, a sole known return operation in an open returns domain does not by itself establish that every relevant return follows that shape.

This also refutes treating “zero closed callbacks/reads/cleanups” as “zero information.” [Certification candidates](../../rust/crates/solid-reactive-ir/src/contract_semantics/certification.rs) explicitly retain selected-call, callback-binding, operation, edge, resource, guard and recursive-value positive facts after withdrawing completeness candidates. [Proof-root derivation](../../rust/crates/solid-reactive-ir/src/contract_semantics/proof.rs) authenticates more than nine closure booleans. An accepted row is meaningful authentication, but a weak usefulness metric.

### 2.3 Where runtime and maintenance effort go

For the same September 9 run, summed audit/report stage durations are:

| Recorded stage | Sum across attempts |
| --- | ---: |
| Proposal generation | 637.924 s |
| Demand planning | 87.413 s |
| Witness acquisition | 5,075.527 s |
| Total recorded certification-attempt duration | 5,868.218 s |

These are summed concurrent **slot durations**, not wall time or CPU time. Witness acquisition includes more than probes; its bucket cannot isolate probe overhead. Recorded certification/receipt/publication buckets are tiny and do not establish that authentication, hashing or publication maintenance is free. Acquisition timing is mostly warm-cache work. Of the 418 rows, 362 used a reused proposal, 42 a published graph, five a generated proposal and nine no lane; this is not a cold-start benchmark.

The 3,102 withheld records comprise **1,816 creates** and **1,286 returns** candidates. Reasons are **1,726 census refusals, 1,131 missing recipes, 140 compositions from a withheld dependency, and 105 vetoes that did not complete**. These counts include dependent/repeated proof work. They show effort aimed at closure; they do not show which application rule demanded it.

Historical [ecosystem timing](../ecosystem-benchmark.md) reported 2,692 native project analyses using about 717 CPU seconds, 384 certification executions about 199 CPU seconds, and 2,259 plan/merge calls about five CPU seconds. Those September 2 counters use a different run and accounting from the September 9 stage buckets. They support investigating repeated project/witness work, not subtracting one table from the other.

The [profile experiment](phase21/2026-09-09-certification-profile-timing.md) already found identical planning outputs with release versus debug times of 0.1100/0.7541 seconds and 0.0883/0.8371 seconds. The [machine full-run report](phase21/2026-09-09-machine-full-result.md) improved from 1,570.421 to 513.899 seconds, but explicitly combines profile, cache, code and concurrency changes. Optimized builds and process cleanup are already identified levers; recommending them as a new semantic simplification would overstate this investigation.

Source size is only a maintenance proxy, not a deletion estimate: the normalized contract-model implementation and its submodules occupy roughly 14.6k lines in 11 files, backend certification roughly 60.1k in 19 files, `certify-contract.mjs` roughly 3.6k, and `creates_walk.rs` roughly 1.3k. Counts include tests, comments and existing working-tree changes. Much of this code protects artifact identity or independent proof, which cannot be deleted merely because a rule does not call it directly.

### 2.4 Is untyped implementation analysis the wrong premise?

**Untyped implementation alone is an unnecessarily weak starting point; authenticated implementation remains necessary for behavior.** The historical 1,086 creates census refusals collapse to 146 source sites, about **7.4 recorded refusals per distinct site**. This is evidence of repeated exposure to the same blocker, not proof that all those cases can share one result: different artifact cases and dependency closures can change the premises.

The current tree has already applied much of the proposed declaration-first lever. [ADR 0038](../adr/0038-declared-signature-premise-for-the-creates-census.md) binds declared signatures to exact implementation parameters. The roadmap records an initial census-refusal decrease from 992 to 786 and coercion refusals from 357 to 80; subsequent helper propagation reduces census refusals from 786 to 748 and coercion refusals from 80 to 30. Getter/accessor refusals can increase as traversal gets past an earlier blocker. The mission's roughly 700 historical “compiled JS parameter is any” occurrences are not the current unaddressed frontier.

Use the cheaper evidence in this order:

1. Resolve the installed exact export, selected signature, declaration bytes and artifact case. Resolve actual-to-formal binding through Type Facts, without syntax/name guesses.
2. Establish only the declaration premises a rule-owned predicate needs: callability, value-only parameters, tuple/member selection, primitive coercion inputs, or async completion type where supported.
3. Ask the exact implementation and authoritative dialect/standard-library premises for the remaining behavioral relationship. Escalate only the unresolved call path, member or operation scope; leave missing evidence uncertifiable.

A declared `{ x: number }` does not prove that `x` has no getter. A declared `Accessor<T>` does not prove a runtime reactive source merely because the alias has that name. `<T>(x: T) => T` does not prove identity with the original argument. A `void` return does not prove absence of effects. TypeScript `never` is not, by itself, a proof that an artifact operation cannot occur. Caller-supplied effects excluded by a creates census disposition cannot be excluded from a read, write or owner question without its own proof.

### 2.5 Which granularity is load-bearing?

| Granularity | Current use | Decision |
| --- | --- | --- |
| Exact package/version/integrity and installation | Active binding and certification trust boundary | Retain. No family-wide or name-based trust. |
| Exact public export and callable/member path | Rules bind different exports to different behaviors; parameter/return paths are already needed | Retain exact identity. Deduplicate proof work by authenticated implementation subject only when every premise matches. |
| Artifact case / export condition | Active exclusive runtime/declaration selection | Retain. Similar runtime behavior does not permit merging artifact identities. Share evidence internally only with separately checked bindings. |
| Signature/actual parameter binding | Selects callback, read and returned-argument subjects | Retain. Declarations are especially valuable here. |
| Call-site guard partition | Rich API has no current ordinary-rule caller | Stop eagerly solving unused partitions, but do not erase conditions. An unresolved selection joins possible alternatives or remains uncertifiable. |
| Operation cardinality and collection completeness | Encoded/proved, then incompletely preserved by projection | Retain when needed to distinguish a possible effect, guaranteed value or exhaustive callback behavior. Ignoring them is a consumer problem, not safe simplification. |
| Full causal/resource graph | Most detail has no current projection; owner source/capabilities have a small active subset | Demand the relevant owner/value relationship. Defer unrelated graph acquisition; keep proof dependencies that justify the selected relationship. |
| All nine domains closed for every export | No rule asks this | Delete as a usefulness goal and eager acquisition requirement. |

### 2.6 Probe and evidence machinery: marginal value

[The evidence/probes design](../package-contract-evidence-probes-design.md) explicitly describes a legacy inferred/probed/reviewed proposal; it is not the current stable `schemaVersion: 1`, acceptance-policy-2 contract model. A finite observed run is a contradiction veto, never a behavioral proof or domain closure.

Today's [synthesized vetoes](../../rust/crates/solid-facts-backend/src/contract_certification/synthesized_vetoes.rs) include a return-undefined observation and a creates observation based on new own keys on `globalThis`. The latter is not a complete observation of owner/reactive resource creation, and the source documents its limited claim. Closure authority comes from the independently verified census, not from seeing no such keys.

For current root catalogs, the direct closure benefits available to attribute are 100 empty creates domains and two empty returns domains. Those can reduce SC9005 and supply dependency premises. They do not establish callback timing, owner transfer or reactive-source identity. Positive facts are already a separate certification route. There is no retained ablation showing how many sound consumer findings the mandatory veto stage added or prevented. The 105 incomplete vetoes and 1,131 missing-recipe records measure withheld work, not defects prevented. No honest findings-per-probe ratio can be calculated from them.

The recommendation is to **stop requesting low-value closures**, thereby avoiding their probe and census work. It is not to issue the same closure receipt after silently skipping its mandatory probe gate. Removing or weakening a gate requires a separate acceptance-policy decision and evidence that the remaining proof establishes the same claim. Preserve tamper, contradictory observation, source mutation, exact-case and issuer tests for every gate still in use.

### 2.7 Replace the usefulness metric

Keep authentication, entrypoint coverage and domain-closure counters as separate engineering measurements. Add a rule-facing measure:

- For a fixed consumer workload, count exact **rule proof obligations** that need package behavior. Record rule, site, artifact identity/case/export, semantic subject, predicate, and required claim knowledge/cardinality. This is a consumer demand record; only the verifier can derive the associated **proof demands**.
- Report **discharged**, **uncertifiable**, and **proved inapplicable** obligations separately. An unqueried export is unqueried, not certified safe. Domain closure never transfers to siblings.
- Measure contract-enabled **violations**, contract-enabled **certified conclusions without violations**, and changes in **uncertifiable results** separately. SC9011 turning into SC9005 is not success.
- Compare contracts present versus absent on the same workload with the same Type Facts, compiler facts, dialect and binaries. Require zero duplicate TypeScript claims against the exact published types. This counterfactual experiment is proposed future work, not an ecosystem rerun performed here.
- Divide useful discharged obligations by unique proof subjects acquired, witness/project-analysis time and maintenance slices. Also report occurrence-weighted reach, so one heavily used primitive is visible without turning package popularity into a proof premise.

The immediate measurement unit should be a rule-owned predicate such as “this selected returned member is this reactive source,” “every possible invocation of this actual callback is deferred,” or “this operation requires the caller's owner.” It is smaller than whole-export closure, but never smaller than the scope needed for that assertion to be sound.

## 3. Ranked designs

### Option 1 — Demand-scoped behavioral proofs over the existing acceptance kernel

**Rank: 1, recommended under the clarified product requirement.** Keep all 44 rule identities and the current exact artifact and proof boundaries. Replace eager all-export behavioral acquisition and the lossy compact consumption interface with rule-owned queries for specific semantic subjects, including missing consumers needed to recognize third-party primitive misuse under existing rules.

**Proves:** the same selected reactive identity, read, callback execution or caller-owner predicate a rule needs, with its required claim knowledge, guard and operation cardinality. A declaration can close a type/binding premise; it cannot substitute for runtime behavior. Positive, partial and closed knowledge remain distinct. A guaranteed returned identity requires an exhaustive relevant return relationship, not just one possible return node.

**Gives up:** automatic pursuit of an export's unrelated behavioral domains and “every export fully described” as a product promise. A predicate needed to extend an existing rule across a package boundary is relevant even if its consumer is not implemented yet. Additional invalidation/error/disposal analyses remain uncertifiable until a concrete rule demand and sound supplier exist.

**Deletes or replaces:** eager requests/reporting/gates for domains confirmed unnecessary by both the current inventory and §1.5's target demands; `exportsProven` as the target; broad import-level missing-field fan-out as the primary interface; the vestigial Throws check; and eventually the compact callbacks/returns/read projection where it loses the demanded subject. It does not delete rules, the accepted decoder, independent verifier, artifact acquisition, receipts, dialect owners or relevant negative census. The four currently unused domains are candidates for this check, not an unconditional deletion instruction.

**Work/value estimate:** removing four unused columns removes 35,800 domain slots from the historical 8,950-export reporting universe, or 49,968 from the current 12,492 accepted-occurrence universe. These are bookkeeping counts, not 49,968 avoided executed proofs. Merely changing the interface adds **zero** behavioral proofs. Existing material offers 616 projected positive occurrences across 82 rows and 48 open-composite return occurrences to investigate, with the cardinality caveat above. A selected predicate can be useful while eight sibling domains stay open; “one predicate versus nine domain closures” is a design-size comparison, not a measured ninefold speedup or labor estimate.

**What becomes uncertifiable that is certifiable today:** no currently demanded, soundly supported predicate should be intentionally lost. All currently accepted exact-artifact facts remain readable. Newly requested dormant-domain certificates are no longer produced. On renewal, empty creates/returns outside demand may no longer be acquired; that loses optional certificates and possible composition opportunities, not a known enabled defect branch. If a dependent selected proof needs N, that makes N demanded and retains its census. Any current conclusion relying on a lost guard/path/cardinality premise must become explicitly uncertifiable until repaired; preserving it would violate the mission's soundness constraint.

**Migration order:**

1. Pin the existing B/C/D/T/A/O/N behavior in a small set of **accepted** consumer fixtures, including type-valid positive, negative, unresolved and artifact/dialect mismatch controls. Add the missing third-party predicate requirements in §1.5 to the implementation inventory. Add a consumer demand ledger alongside existing diagnostics before changing suppression or receipt policy.
2. Replace one consumer slice at a time: returned reactive identity and selected member paths; direct/parameter reads; callback timing plus completeness; caller-owner requirements. Preserve guard, actual-to-formal binding and operation cardinality at each slice. Use the existing instantiation machinery where its proof semantics fit; do not invent a second normalized model.
3. Use those recorded rule needs to select proposal candidates. The verifier still derives all proof demands for each selected candidate, including dependencies, initialization authority, negative premises and mandatory vetoes. Callers cannot drop inconvenient demands.
4. Stop active acquisition only for domains that neither current consumers nor the required third-party extensions need. Demand declarations first, then exact implementation evidence only for the unresolved predicate. Cache by complete artifact/dialect/premise identity; never by export name alone.
5. Move SC9005 to the unmet rule obligation and retain an explicit certification summary. Remove the SC9011 “binding exists therefore enough” shortcut. Retire the old primary metric and duplicate import noise only after unchanged certainty has been demonstrated.

The stable public schema remains backward-compatible. Optional omissions mean unknown only as already specified. Removing fields from historical receipts, changing a claim's meaning, or narrowing the acceptance policy requires deliberate version/policy migration and fresh receipts; no reinterpretation of stable version 1 is proposed.

### Option 2 — A small portfolio of fully proved, exact primitive contracts

**Original rank: 2; not selected after the product clarification.** This remains a cost comparison, but a permanently restricted portfolio does not satisfy the intended direction of retaining the full rule set with expanding third-party primitive coverage. A small initial implementation slice is still useful; it is not a permanent support ceiling. This option uses the same sound proof requirements as option 1 but deliberately supports a finite set of observed consumer needs.

**Proves:** selected predicates for a maintained set of exact exports/artifacts/cases. Author a small machine-replayable proof or supported static derivation for each implementation motif; human review and finite probes alone are not proof. Start with callback timing, returned reactive sources/selected members and cleanup owner requirements. Retained `@corvu-next/utils@0.1.5` callback rows and the broadcast-channel owner/member examples are concrete candidates to rank against actual usage, not a recommended trust list.

**Gives up:** automatic coverage of every corpus package and automatic recovery through every unfamiliar module/export shape. Unknown versions, unsupported cases and out-of-portfolio primitives stay uncertifiable. “Fewer packages, fully proved” is preferable when no measured application demand justifies the long tail.

**Deletes:** the broad product generation/recovery path once it is no longer supported, including the no-demand graph-retry and partial-proposal preparation orchestration in `certify-contract.mjs` and its dedicated recovery fixtures. The particular `preparePublishedGraphCases`/partial-proposal/retry machinery must first be separated from exact acquisition still needed by selected proofs. Keep a small certification tool and kernel, pin checks, declaration acquisition and the proof/census capabilities the portfolio actually uses. Do not delete every dependency walk: even a small primitive can depend on another package.

**Work/value estimate:** each maintained slice must enable at least one demonstrated rule predicate on a type-valid consumer case. The historical 1,086/146 repetition suggests reuse opportunities at exact implementation subjects; it does not guarantee 7.4 times less work. The present upper bound on existing positive projection material at risk is 616 occurrences across 82 rows. No retained usage counts justify choosing a package quota or estimating engineer-days. Stop adding a portfolio slice when its upkeep exceeds its measured discharged-obligation value; do not loosen its proof.

**What becomes uncertifiable that is certifiable today:** new publications and new versions outside the portfolio lose automatic authentication and any currently available behavioral or empty-domain facts. If existing selected catalogs are retired, some or all of the 616 projectable positive occurrences and 102 root domain closures outside the selected set are lost. Historical accepted artifacts can remain usable under their exact existing policy; this avoids an abrupt capability regression but does not maintain future-version coverage.

**Migration order:** record actual consumer demand; choose and fully exercise the first three predicate families; retain exact legacy publications while renewal support narrows; publish the explicit unsupported frontier; then remove the unused broad recovery path and its exclusive tests. Never count an out-of-portfolio absence as a clean certificate.

### Option 3 — Keep the expressive pipeline and finish its consumer

**Rank: 3; justified only by an explicit near-term need for the richer analyses.** Keep the general normalized operation graph, all nine representable claim domains, certification/evidence system and ecosystem generator. Redirect the next work from creates closure count to consumer adoption and useful positive predicates.

**Proves:** the current expressive contract model, with a rule path that finally asks exact guard/cardinality-aware queries. Retains headroom for future disposal, invalidation and error rules without pretending those rules exist today.

**Gives up:** much of the desired maintenance reduction. Broad module acquisition, recovery, proof families and the schema surface remain. Improving empty creates alone still cannot enable most of the active contract branches.

**Deletes:** the all-nine-closed success target, duplicate no-demand scheduling where proven redundant, vestigial Throws/async plumbing, and the lossy intermediate projection after migration. Unlike option 1, it retains the currently unused domain representation and general acquisition capabilities deliberately as future investment.

**Work/value estimate:** the immediate ceiling is the same existing 616 projectable occurrences and 48 composite-return opportunities, not thousands of newly guaranteed facts. Guard-aware consumption may initially increase uncertifiable results because it stops treating partial information as stronger than it is. The declaration-premise and optimized-build levers are already partly implemented; there is no evidence that one more generic creates-census case alone changes the product's yield. The lever with direct rule relevance is **complete one demanded consumer predicate and its acquisition proof end to end**, then measure its verdict delta.

**What becomes uncertifiable that is certifiable today:** no intentional loss of legitimately supported facts or domain certificates. Conclusions whose current projection discarded a required premise must become uncertifiable until that premise is supplied, as in option 1. There is no option consistent with the constraints that promises to retain those conclusions merely to avoid a regression in counts.

**Migration order:** first preserve certainty through the consumer; restore real accepted-contract fixture coverage; measure rule demands and counterfactual yield; acquire the missing predicates; only then fund new behavioral domains against a named rule. If that roadmap does not exist, retaining four dormant acquisition surfaces is not justified by this inventory.

**Rejected standalone alternative: declarations-only behavioral contracts.** This would retain some binding/type facts and native async classification, but give up runtime reactive identity, callback timing, owner transfer and cleanup obligations. It cannot provide equivalent soundness and capability for the active R columns. It is a useful first acquisition stage, not a ranked replacement.

## 4. Proposed deletion ledger

“No direct rule consumer” is sufficient to question active behavioral feature work, but insufficient to delete a proof dependency or trust-boundary test. The ledger separates those cases. These are proposed changes for a later implementation, not changes made here.

| Delete / retire | Concrete location or scope | Condition and retained boundary |
| --- | --- | --- |
| Active eager acquisition and success gates for domains with no current or required third-party consumer; candidates are `writes`, `invalidates`, `throws`, `disposals` | Domain scheduling, candidate generation/report columns and domain-only assertions around `contract_semantics`, `demand_plan`, proposal generation and certification fixtures | First settle §1.5's missing consumers. A needed third-party write/action capability blocks deletion of its required supplier. Preserve decoding/validation/canonical hashing of historical stable documents until an explicit compatibility migration. Do not rewrite accepted receipts to drop fields. |
| All-export/all-nine `exportsProven` as the primary accuracy criterion | `accuracy-roadmap.md`, ecosystem report and goals derived from it | Replace with discharged rule obligations and verdict/uncertifiable deltas. Historical raw closure counts can remain labeled. |
| Vestigial Throws check for async incompleteness | `contracts.rs::push_unknown_contract_claims` | A is derived from returns; no ordinary projection inserts Throws. Keep actual async incompleteness reporting. |
| Blanket missing-field import fan-out as the product proof-demand interface | `push_unknown_contract_claims` and downstream diagnostic accounting | Replace only after rule-site obligations preserve all uncertifiable cases. A quieter import is not a certification. |
| Lossy compact behavior adapters | `project_callbacks`, parameter-read tuples, `project_return_shape`, associated string-based execution/owner adapters | Replace with narrow normalized queries carrying needed subjects and certainty. Do not delete guard/cardinality semantics because the current adapter ignores them. |
| Eager creates/returns census and veto work for unqueried exports | `creates_walk.rs`, candidate planning and `certify-contract.mjs` scheduling | Retire scheduling, not the census kernel needed for demanded negative or compositional proofs. This is where executed-work savings should be measured. |
| Redundant obsolete-policy consumer expectations | The 16 reactive fixture directories below | Migrate intended positive cases to actual accepted artifacts first; preserve a focused old-policy rejection fixture. Do not present stale fixtures as evidence that current positive consumption works. |
| Broad recovery-only product code and fixtures | Graph/partial-proposal retry orchestration and its exclusive tests | **Option 2 only**, after exact supported proof acquisition is independent of it. Options 1/3 may still need it on demanded paths. |
| Legacy evidence-tier design as current guidance | `docs/package-contract-evidence-probes-design.md` | Keep as explicitly historical design material or archive it; do not revive inferred/probed/reviewed authority tiers. |

The [contract corpus](../../fixtures/package-contracts/corpus.json) currently lists **96 fixtures**. No complete corpus fixture is demonstrated deletable solely by this consumer inventory: a fixture can test independent proof, artifact identity or rejection behavior while its modeled operation is not directly consumed by a rule. Remove dormant-domain-specific expectations where they have no remaining proof responsibility, and retain the shared fixture's live assertions.

The separate reactive consumer fixtures contain **19 obsolete acceptance-policy-1 contract entries in 16 directories**: `package-variant-precedence-consumer`, `package-store-destructure`, `package-contract-install-shapes`, `package-callback-consumer`, `package-parameter-member-consumer`, `package-callback-arguments-consumer`, `package-consumer`, `package-structured-return`, `package-unknown-export`, `package-return-consumer`, `v1-reactivity`, `package-store-consumer`, `package-unknown-callback-consumer`, `package-contract-paths-shadow`, `package-unknown-returns-consumer`, and `package-structured-unresolved` under `fixtures/reactive-ir/`. Their old intended positive semantics are not current authentication. Consolidating rejection coverage is reasonable; deleting the intended behavioral coverage is not.

**Retain:** integrity/pin and exact-case gates; independent acceptance and evidence replay; Type Facts provenance; dialect/initialization authority; TypeScript-oracle cases using real package types; shadowed/namespace/member/wrapper controls; contradictory probe and mutated-source tests; callback completeness and owner-capture counterexamples; and unknown dependency fail-closed tests. These outputs protect the facts rules consume even when a rule never directly reads them.

## 5. Open questions and decision criteria

1. **Actual application demand is unmeasured.** Which external exports are called in real consumer workloads, and which missing predicates block rule conclusions there? Retained certification audits cannot answer this. Add a demand ledger and a bounded consumer ablation experiment before claiming a tenfold saving or ranking packages by value.
2. **How many projected positive facts already change sound conclusions?** The 616 count is a projection-shape tally. All recorded operation cardinalities permit zero executions; no retained end-to-end evidence links those occurrences to discharged rule obligations. Prioritize guard/completeness/subject preservation and accepted consumer fixtures before assigning finding credit.
3. **Can the 48 open composite returns answer member-level demands?** The model contains positive members, but selected-path identity, possible alternative returns and initialization/getter behavior must be checked. The count is a bounded candidate set, not 48 promised repairs.
4. **What missing callback owner facts matter most?** The current 145 callbacks contain no owner source. Timing-only facts cannot certify owner propagation. A named application case is needed to choose captured/ambient/created owner proof work.
5. **What is probes' isolated marginal benefit?** Current timing bundles witness acquisition; no controlled ablation identifies contradictions missed by static certification or findings preserved by vetoes. Do not remove mandatory gates from an otherwise identical certificate based on absent measurements.
6. **Which historical roadmap inputs produced the 54-open-claim discrepancy?** The displayed proposal table and total disagree. The September 9 accepted recount uses a separate, explicit denominator and does not resolve that history.
7. **Which dormant fields can physically leave the public model?** Stable-version compatibility and canonical receipt identities constrain deletion. Active acquisition can stop first; wire/model deletion needs a separately scoped version and policy decision.
8. **Which current rule certainty gaps are reproducible?** The guard/path/cardinality and related soundness-memo observations require focused positive/negative/unknown fixtures. This investigation did not execute new semantic reproductions, and it does not treat source concerns as measured false-positive counts.
9. **Which acquisition costs remain after demand filtering?** The same exact artifact/closure may still need one initialization and Type Facts analysis for a single exported predicate. Savings will not be proportional to removed output columns. Measure shared setup separately from incremental predicate acquisition.

Proceed toward option 1 while retaining the full rule set and third-party misuse as a central acceptance criterion. Establish the approach with a small accepted consumer slice, then extend the missing rule/package connections; the slice must not become a permanent portfolio restriction. Keep option 3 as an architectural alternative if concrete rule demands justify its full representation. None of those decisions needs another ecosystem benchmark to establish the demand mismatch documented here.

## Appendix A. Measurement provenance and read-only recount

The completed source report is `rust/target/ecosystem-investigations/2026-09-09-async-binding-full.json`:

```text
SHA-256: 6e8d6bcba4d24eb8488128d3afd3fc0ed417732031badaccc58312a1baa14b71
Finished: 2026-09-09T01:47:52.964Z
Duration: 534042 ms
Selected publication tuple digest:
757d77771abb2ca3f3c1f44ce28690b1eb81b5b7dee5c3bebf5315c2285835db
```

The last digest hashes compact JSON of sorted `(probeId, artifactCaseId, documentDigest, receiptDigest)` tuples; it is a recount fingerprint, **not an acceptance proof root**. The linked phase21 measurement preserves before/after report identities and selection counts. Its comparison annotations are not receipt authority. Retained output directories live in temporary/build storage and may be cleaned; the report hash alone does not preserve those bytes.

This core read-only recount reproduces the root-domain table and fingerprints while checking selected document/receipt digests. It excludes non-selected publications, dependency catalogs and incomplete runs. It requires the retained paths referenced by that report; it neither installs packages nor invokes the checker.

```python
import collections, hashlib, json
from pathlib import Path

report_path = Path("rust/target/ecosystem-investigations/2026-09-09-async-binding-full.json")
assert hashlib.sha256(report_path.read_bytes()).hexdigest() == \
    "6e8d6bcba4d24eb8488128d3afd3fc0ed417732031badaccc58312a1baa14b71"

def read(path, digest=None):
    raw = path.read_bytes()
    if digest:
        assert "sha256:" + hashlib.sha256(raw).hexdigest() == digest, path
    return json.loads(raw)

domains = "callbacks reads writes creates invalidates throws returns cleanups disposals".split()
closed, items, positive = (collections.Counter() for _ in range(3))
totals, publications = collections.Counter(), []
for row in read(report_path)["results"]:
    if (row.get("certificationAttempt") or {}).get("status") != "certified":
        continue
    output = Path(row["retainedArtifacts"]["outputDir"])
    roots = list(output.glob("*.accepted-catalog"))
    assert len(roots) == 1
    root = roots[0]
    if (root / "accepted-contract-case-set.json").exists():
        pointer = read(root / "accepted-contract-case-set.json")
        selected = root / pointer["document"]
        cases = read(selected, pointer["documentDigest"])["cases"]
    else:
        selected = root / "accepted-contracts.json"
        direct = read(selected)["contracts"]
        assert len(direct) == 1
        cases = [{"catalog": selected.name,
                  "artifactCaseId": direct[0]["bindings"]["resolvedImportRoot"]}]
    totals["rows"] += 1
    for case in cases:
        catalog_path = selected.parent / case["catalog"]
        entries = read(catalog_path, case.get("catalogDigest"))["contracts"]
        assert len(entries) == 1
        entry = entries[0]
        main = read(catalog_path.parent / entry["document"], entry["documentDigest"])
        read(catalog_path.parent / entry["receipt"], entry["receiptDigest"])
        publications.append((row["probeId"], case["artifactCaseId"],
                             entry["documentDigest"], entry["receiptDigest"]))
        totals["cases"] += 1
        for ep in main["entrypoints"].values():
            for artifact in ep["cases"]:
                for summary_id in artifact.get("exports", {}).values():
                    call = main["summaries"][summary_id].get("call", {})
                    totals["exports"] += 1
                    totals["all9closed"] += all(d in call.get("closed", []) for d in domains)
                    totals["withPositive"] += any(call.get(d, []) for d in domains)
                    for d in domains:
                        closed[d] += d in call.get("closed", [])
                        items[d] += len(call.get(d, []))
                        positive[d] += bool(call.get(d, []))
print(dict(totals))
for d in domains:
    print(d, "positiveExports", positive[d], "items", items[d], "closed", closed[d])
print(hashlib.sha256(json.dumps(sorted(publications), separators=(",", ":")).encode()).hexdigest())
```

The projection tally additionally inspected operation outputs, callback origins/schedules/tracking/owner sources, read inputs and cleanup owner requirements against the functions in §1.1. It is intentionally labeled a source-based tally, not an executed analyzer test. No ecosystem benchmark, package installation, contract generation, snapshot update or independent certification was performed for this investigation.

## Appendix B. Handoff verification

Document checks passed: all 44 rule identities occur in the matrix, all 77 local links resolve, the embedded recount reproduces the root-domain table and selected-publication fingerprint, and whitespace is clean. The repository's universal handoff checks also passed: Rust formatting, `git diff --check`, schema JSON parsing, validation of both dialect manifests, and workspace/all-target Clippy with warnings denied (offline).

No production code, fixtures, snapshots, manifests, accepted contracts or receipts were changed. No focused semantic fixtures or test suites were run: this pass added a proposal, not a semantic implementation. Full `make verify`, coverage/ownership comparisons, TypeScript-oracle execution and ecosystem benchmarking were intentionally not run. Existing retained results are evidence only for the runs identified above; implementation, projection certainty repairs and the exact remaining uncertifiable cases in §5 remain proposals.
