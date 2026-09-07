# Solid Checker

Solid Checker certifies the reactivity and asynchronous behavior of Solid
TypeScript projects without coupling its analysis to one compiler backend.

## Language

**Type Facts**:
Compiler-independent semantic facts about a configured TypeScript project.
_Avoid_: Compiler facts, checker data

**Async function fact**:
A semantic summary of a function-like declaration or expression, including whether it can return asynchronously and which calls are dominated by an await.
_Avoid_: Async scan result, async metadata

**Reference index**:
The generation-scoped mapping from durable symbol identities to their source reference locations.
_Avoid_: Reference cache, usage map

**Type Facts session**:
A retained analysis lifetime for one configured TypeScript project, carrying its current generation and acknowledged demand state across requests.
_Avoid_: Lifecycle responder, retained protocol state

**Type Facts producer**:
The repository-owned process that answers Type Facts demands for one configured TypeScript project. It stays behind the versioned process/session protocol.
_Avoid_: Server, backend, external sidecar

**Generation**:
A numbered Type Facts project state. Every accepted update advances it exactly once; generation-scoped identities must be reacquired afterward.
_Avoid_: Revision, snapshot

**Semantic demand run**:
One source file's canonically ordered Type Facts demands for a generation.
_Avoid_: Query batch, demand group

**Wire table transition**:
A deterministic transport frame that establishes a Type Facts table or transforms the table named by its base generation and state token.
_Avoid_: Payload, packed delta

**Configured source descriptor**:
A Type Facts session source entry that names a canonical disk path for local hydration, while edited or virtual sources remain inline. Generation source hashes still prove that Rust and TypeScript-Go analyzed identical content.
_Avoid_: Path-only source, source shortcut

**Semantic lookup**:
The project-wide query surface rule discovery asks for semantic answers — the entity or symbol at or containing a location, the function a symbol names, whether an owner is rendered under a Loading boundary — instead of scanning fact tables.
_Avoid_: Index helpers, fact-table scan, range-query module

**Fact domain**:
One of the independent evidence suppliers the checker cross-references: Oxc syntax facts, Solid compiler execution facts, Type Facts, and package contracts. User-facing documentation may call them "sources of evidence"; the canonical term is fact domain.
_Avoid_: Backends, sidecars, analysis inputs

**Compiler semantic trace**:
The compiler-owned, transform-local record of semantic decisions made while lowering one source file. It observes existing lowering and is versioned separately from the normalized compiler execution facts consumed by the checker.
_Avoid_: Execution map, compiler sidecar, instrumented output

**Compiler execution facts**:
The checker-normalized fact domain projected from a validated compiler semantic trace, binding source sites to actual execution, callback, ownership, and discarded-code decisions. Its protocol version is independent of the producer trace version.
_Avoid_: Type Facts, compiler predictions, semantic trace (that is the producer form)

**Claim domain**:
One independently knowable set of package behavior, such as callback invocations, reactive reads, or cleanup operations. Its knowledge may be open or closed without determining any sibling claim domain.
_Avoid_: Fact domain, section, field group

**Domain closure**:
Accepted proof that one claim domain enumerates every behavior possible for its exact package, export, artifact case, guard, and resource scope. Closure never transfers implicitly to a parent, child, or sibling claim domain.
_Avoid_: Complete contract, tested absence, empty result

**Claim knowledge**:
The local knowledge state of one claim domain: unknown, partial positive, complete positive, or complete negative. A state says nothing about a parent, child, sibling, alternative, or referenced resource unless that subject has its own claim knowledge.
_Avoid_: Status wrapper, inherited completeness, contract completeness

**Operation cardinality**:
A proved lower and upper bound on how often one semantic operation occurs within an explicit scope such as one trigger occurrence, one package call, or one resource lifetime. Possibility, guarantee, repetition, and bound scope are separate facts.
_Avoid_: Call count, observed frequency, phase repetition

**Guard partition**:
A finite set of statically decidable contract alternatives selected from exact call, value, protocol, or artifact facts. A complete partition is proved disjoint and exhaustive; unresolved selection joins every possible alternative.
_Avoid_: Runtime condition, source expression, heuristic branch

**Contract proposal**:
An unaccepted machine description of package behavior together with proof obligations. A proposal cannot certify a project or discharge a proof obligation.
_Avoid_: Generated contract, inferred contract

**Accepted package contract**:
A normalized package contract whose closed claims, package identity, artifact cases, and proof inputs are bound by a valid acceptance receipt. Open claims remain usable only as partial knowledge.
_Avoid_: Verified JSON, trusted contract, reviewed contract

**Built-in runtime model**:
The selected Solid dialect's reviewed premises about `solid-js`,
`@solidjs/signals`, and `@solidjs/web`. Ordinary analysis uses these premises
without core package contracts. Model selection is neither an independent
implementation certificate nor authentication of installed runtime bytes;
unmodelled behavior remains unknown.
_Avoid_: Certified Solid runtime, bundled core receipt, trusted package name

**Normalized contract model**:
The rich, wire-independent semantic representation produced by the single contract decoder and normalizer. Reactive IR consumes this model and never compact-document omission, summary, closure-array, or schema-version conventions.
_Avoid_: Expanded JSON, decoded schema, contract AST

**Artifact case**:
One exact package-entrypoint resolution outcome, including its resolution trace, runtime artifact, declarations, and dependency closure. Artifact cases are selected exclusively and never merged.
_Avoid_: Environment, mode, variant

**Operation graph**:
A causal description of package behavior whose nodes are semantic operations and whose edges express scheduling, data, cleanup, error, or lifetime relationships.
_Avoid_: Phase list, callback list, execution trace

**Evidence sidecar**:
A hash-bound artifact containing detailed fact transcripts, probe observations, and proof material for package-contract claims. Ordinary project analysis does not need it after acceptance.
_Avoid_: Contract evidence field, audit log

**Proof demand**:
A verifier-derived, policy- and artifact-snapshot-bound requirement for one exact claim or positive fact, assigned to its authoritative fact owner. Callers may transport its opaque identity but cannot create, omit, satisfy, or declare it inapplicable.
_Avoid_: Proof checklist, caller obligation, requested evidence field

**Artifact snapshot**:
The immutable content-addressed view of one independently acquired package artifact used for every read during a certification transaction. It is distinct from an artifact case (a selected resolution outcome) and from a Type Facts generation (a project-analysis state).
_Avoid_: Package directory, temporary extraction, artifact case, generation

**Probe gate**:
A verifier-derived mandatory contradiction veto for one proposed closed claim. A contradiction blocks closure; success or finite non-observation never proves absence, completeness, or safety.
_Avoid_: Runtime proof, passing probe, negative observation

**Implementation census**:
The proof mode that decides a closed behavioral call domain from the demanded export's own runtime implementation transcript rather than from its declaration: every transcript is complete with no control-flow marker and its declaration node (bound from the authenticated bytes) contains no jump, every invoking form the producer walked is enumerated, every call at the `MayExecute` floor is assigned exactly one **census disposition** (`unreachable`, `parameter-rooted`, `parameter-rooted-accessor`, `standard-library`, `dialect-axiom`, `local-recursion`, `local-recursion-backedge`) — the accessor disposition also covers a read accessor whose subject the producer roots at an unwritten parameter of the frame (ADR 0034) — and the first premise that fails refuses the domain by name. It decides `creates`, and — reading the same transcript's completions instead of its calls — the empty `returns` closure (ADR 0035): a plain, block-bodied implementation whose classified control-flow census carries no value-carrying return site at the `MayExecute` floor; an `async` function, a generator, an expression body, and a reachable or unknown-reach value return refuse by name.
_Avoid_: Call scan, callee check, behavioral census, implementation walk (that is the generator's proposal walk)

**Withheld closure candidate**:
A proposed closed claim domain the certifier withdrew by name and re-planned without: a `creates` or `returns` candidate whose semantic claim no recipe (hand-authored or synthesized) can serve (`no recipe in corpus`), one the implementation census could not decide (`census refused: …`, ADR 0036), or one whose mandatory veto run ended in an error or a timeout (`veto did not complete: gate …`). The domain is opened in the certified contract and the withholding is recorded with its reason in the certification audit. It is neither a refusal nor a certification of the closure; a veto *contradiction* is never a withholding and refuses the row.
_Avoid_: Skipped claim, dropped gate, waived probe, refused closure

**Recipe corpus**:
A directory of claim-addressed probe modules plus its manifest, supplied to one certification transaction and keyed by exact semantic claim id — hand-authored, or synthesized by the checker from an export's Type Facts call signature for a candidate no hand recipe addresses (ADR 0036; `provenance: synthesized`, a hand recipe always wins). It is an **input**, never a root of trust: omitting a scheduled gate refuses that gate, a vacuous recipe only fails to veto, and the verifier derives every module's construction digest from the bytes it copied. A corpus inside the analyzed package is refused outright.
_Avoid_: Probe suite, test corpus, trusted recipes, probe fixtures

**Detect-and-refuse isolation**:
The Stage 1 probe scheme: instead of denying a probe's writes, the verifier proves none of the inputs the rest of the transaction reads was disturbed — a private snapshot copy, a watched census re-hashed before, between, and after launches, and refusal of any resolvable `node_modules` above the private directory — and refuses the gate when it cannot show that. It denies nothing, including network access, and its recorded sandbox policy digest says so in its own field list.
_Avoid_: Sandboxed probe, isolated execution, denied writes, probe sandbox (that is Stage 2's OS-level scheme)

**Verified positive fact**:
An analyzer-visible possible behavior retained only after its exact proof demand has an authoritative witness. It does not close its surrounding claim domain or imply that unobserved sibling behavior is absent.
_Avoid_: Partial proof, inferred behavior, closed claim

**Acceptance receipt**:
A verifier-issued binding among a contract's wire and semantic identities, exact artifacts, proof material, and verification policy. It is the authority ordinary analysis uses to accept closed claims.
_Avoid_: Verification report, signature, trust flag

**Issuer provenance**:
The authenticated channel through which a receipt gains trust: compiled
built-in bytes, a configured persistent-local issuer, or an explicitly trusted
portable issuer chain. A key ID, public key, or signature carried only inside
the receipt is never issuer provenance.
_Avoid_: Self-signed receipt, receipt key, trusted signature

**Semantic digest**:
A canonical identity of normalized package-contract meaning that excludes wire version, summary names, formatting, and evidence layout.
_Avoid_: File hash, contract hash

**Rule options**:
The project-level per-rule configuration document, `.solid-checker/rule-options.json`, discovered beside a project's contracts and carrying the upstream eslint-plugin-solid options the 1.x rules honour. Defaults are upstream's defaults; parsing fails closed. Part of every build and diagnostic identity.
_Avoid_: Rule config, checker settings, options file

**Finding kind**:
Whether a finding is a **violation** (the analyzer proved the code misbehaves at runtime) or **uncertifiable** (a proof obligation the analyzer could not resolve). Distinct from severity (error/warning).
_Avoid_: Finding status, finding type

**Reproduction condition**:
An export condition the probe harness adds to a launch's `--conditions=` flags beyond the artifact case's *requested* conditions, so the pinned interpreter selects the runtime files the certified closure resolves (ADR 0037). Drawn from a fixed constant (today `browser`) and admitted only when every planned case and closure edge reproduces under the resulting applied set and every `exports`/`imports` object in the authenticated closure selects identically. Recorded as `reproduction:<c>` beside `requested:<c>` and `esm:<c>`/`require:<c>`.
_Avoid_: extra condition, fallback condition, browser mode, condition override

**Declared-signature premise**:
The condition under which an export's `creates` census classifies the invoking forms of its root implementation since ADR 0038: each parameter of the JavaScript implementation carries the type the export's *declared* call signature (the `.d.ts` the consumer compiles against, and the signature the synthesized veto samples from) gives that position, established on a checked twin of the file, instead of the implicit `any` of an unannotated parameter. Stated by the producer as `parameterPremises`, bound by the verifier to the one declared signature byte for byte, and recorded as `census-premise:` witness sites. It changes only the type-decided forms (coercion, iteration, `await`); it is never evidence about accessors.
_Avoid_: typed census, inferred parameter types, declared types as facts (they are a stated condition)

**Parameter-rooted accessor**:
A property or element access whose receiver the producer roots at a plain, unwritten parameter of the censused declaration, dispositioned by the `creates` census instead of refusing (ADR 0034) because the getter, setter or trap it can run was installed by the caller on an object the caller passed. Since ADR 0040 the disposition holds in **write** position too, and the form states `subjectWrite` so the two are separable: the receipt records `parameter-rooted-accessor` or `parameter-rooted-accessor-write`. The write flag exists for the domains that must refuse it — for `writes` and `invalidates` the assignment is this export's own operation, whoever wrote the accessor.
Since ADR 0041 the same premise covers two more forms, by naming their subject differently: an object or JSX prop spread's is its operand, and an object pattern binding element's is the value the outermost enclosing pattern destructures. ADR 0042 adds the iteration protocol (`parameter-rooted-iterable`) and the callee a `for…of` head binds from such an iterable (`parameter-rooted-element`), and separately proves that a **rest parameter's** array is the engine's, so iterating or spreading it records no form at all — a syntactic fact its `any[]` type cannot supply. `for await…of` is outside all of it.
Since ADR 0043 the **root set** itself is closed under the reads this census dispositions: a name a parameter's own object binding pattern bound, and a name a local declaration bound from an already-rooted initializer (to a fixpoint), are rooted, because naming an intermediate does not change whose value it is. Every stated subject carries `subjectRoot`, the derivation that rooted it, over a closed set — `parameter` for all of the above, and `parameter-default` for the one different claim, a parameter whose default names another rooted parameter, whose value is caller-supplied under either of two branches. An unreviewed or absent derivation refuses.
_Avoid_: safe accessor, inert property, trusted receiver, treating the read and write dispositions as one, treating a defaulted root as a plain one

**Own literal**:
A binding this program initialized from an object or array literal, or from an object pattern's rest element, every own property of which the specification creates with CreateDataPropertyOrThrow (ADR 0044). A form whose subject is a *direct* reference to such a binding — a computed-key element access included, which the checker resolves no symbol for — reaches a data property or the engine's own prototype chain, so the `creates` census dispositions it as `own-literal-accessor`, `own-literal-accessor-write` or `own-literal-iterable`. Not a claim about the caller: the form carries `subjectRoot: own-literal` and `subjectDeclaration` (the binding's range, which the verifier places in the artifact's own runtime source) and never a `subjectParameter`. Resolved through an import when the table is declared in another module. The literal must install no `get`/`set` and no `__proto__` member; the binding must be declared once and written nowhere. What a member *holds* is not covered: `table[k]` clears, `table[k].x` does not. The premise was already taken silently wherever the key is a literal (no form is recorded); this names it.
_Avoid_: constant table, frozen object (nothing here is frozen), plain object (an escape claim this does not make), trusted literal

**JSX-free module premise**:
The condition under which a veto executes a `.jsx` module as ordinary ECMAScript (ADR 0039). A `.jsx` extension is a bundler convention, not a syntax: a package that publishes a `solid` condition names uncompiled sources `.jsx`, and most such modules declare helpers rather than markup. The checker admits a member of an authenticated snapshot only when its own parser reports no JSX element and no JSX fragment; the worker executes exactly the admitted files through one load hook that re-digests each and refuses every other `.jsx` URL by name. The premise stated is that a JSX transform is the identity on a module with no JSX, and the interpreter falsifies it — every JSX form is a syntax error in ECMAScript. Recorded as `jsx-free-esm:` in the probe-gate root.
_Avoid_: JSX support, compiled JSX (that is the separate lever), transpiling in the harness, treating the `.js` sibling as equivalent

**Call-argument premise**:
How a premise reaches a local helper (ADR 0038, helper premises; handshake protocol 23). A premised census records, for each call to a declaration in the program's own runtime source, the type the twin gave every informative written argument slot (`callArgumentPremises`); the verifier copies the entry for the call it follows onto the helper's local-declaration demand (`parameterPremises`), the producer classifies the helper on a spelled twin bound to exactly those types, and the helper's transcript echoes them or states none. A slot the caller's twin typed `any`, or a call carrying a spread, hands the helper nothing for that slot. The same helper reached under two argument-type sets is two demands and two transcripts. Bound byte for byte at every hop, identity included, and recorded as `census-premise:` sites at the helper's span.
_Avoid_: premise inheritance, propagated types, inferred helper signature, union of callers (each call site is its own condition)

**Primitive completion**:
A transcript's answer to whether the value that implementation hands its caller is provably a primitive: the checker's return type for the declaration, over the very program its census was classified on — the premise twin included — is a union of primitive types alone (ADR 0045). It settles the completion form too, since a `Promise` and a `Generator` are not primitives. A *coercion* in a caller states `coercionPremise`, the calls its clearance rests on (every other operand being primitive by its own type), and the `creates` census grants that premise only from each callee's own primitive completion, read off the transcript it demanded under the premise it recorded. Because the fact belongs to the premise, the same helper may state it under one argument premise and not another.
_Avoid_: pure function, returns a number, safe coercion, treating a stated `coercionPremise` as a verdict (it is a statement of what the clearance would rest on)

**Discarded region**:
A source region the Solid compiler censused and then **deleted** — the `Value(Elided)` decision, projected as `ExecutionMap::discarded_regions` and classified `ExecutionRole::DiscardedRendering`. Distinct from an *untracked region*, which is code that executes once at render: a discarded region executes zero times, so it supports no finding and no certification. Silence over one means "both compilers deleted this", never "this was proven safe".
_Avoid_: Elided region, dead region, untracked region (that is the once-executing one), unreachable code

**Failure class**:
A user-facing grouping of the runtime misbehavior that findings prevent: silent staleness (reads that register no dependency), feedback loops (writes and actions in owned scopes), escaped async (pending reads outside tracked or Loading-bounded regions), and lifecycle leaks (effects, cleanups, and boundaries without a live owner).
_Avoid_: Bug category, rule group (that is the SCxxxx numbering)
