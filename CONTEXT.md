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
A Type Facts session source entry that names a canonical disk path for local hydration, while edited or virtual sources remain inline. Generation source hashes still prove that Rust and tsrs analyzed identical content.
_Avoid_: Path-only source, source shortcut

**Semantic lookup**:
The project-wide query surface rule discovery asks for semantic answers — the entity or symbol at or containing a location, the function a symbol names, whether an owner is rendered under a Loading boundary — instead of scanning fact tables.
_Avoid_: Index helpers, fact-table scan, range-query module

**Fact domain**:
One of the independent evidence suppliers the checker cross-references: Oxc syntax facts, Solid compiler execution facts, Type Facts, and dependency source. User-facing documentation may call them "sources of evidence"; the canonical term is fact domain.
_Avoid_: Backends, sidecars, analysis inputs

**Compiler semantic trace**:
The compiler-owned, transform-local record of semantic decisions made while lowering one source file. It observes existing lowering and is versioned separately from the normalized compiler execution facts consumed by the checker.
_Avoid_: Execution map, compiler sidecar, instrumented output

**Compiler execution facts**:
The checker-normalized fact domain projected from a validated compiler semantic trace, binding source sites to actual execution, callback, ownership, and discarded-code decisions. Its protocol version is independent of the producer trace version.
_Avoid_: Type Facts, compiler predictions, semantic trace (that is the producer form)










**Built-in runtime model**:
The selected Solid dialect's reviewed premises about `solid-js`,
`@solidjs/signals`, and `@solidjs/web`. Ordinary analysis uses these premises
without core package contracts. Model selection is neither an independent
implementation certificate nor authentication of installed runtime bytes;
unmodelled behavior remains unknown.
_Avoid_: Certified Solid runtime, bundled core receipt, trusted package name

















**Rule options**:
The project-level per-rule configuration document, `.solid-checker/rule-options.json`, discovered in the project's `.solid-checker` directory. Every shipped rule takes `enabled` and nothing else; an unknown option is refused rather than ignored, so a typo cannot silently mean "defaults". A retired rule name still loads and is a no-op, which is how a document written against the 1.x catalog keeps parsing (ADR 0110). Parsing fails closed. Part of every build and diagnostic identity.
_Avoid_: Rule config, checker settings, options file

**Finding kind**:
Whether a finding is a **violation** (the analyzer proved the code misbehaves at runtime) or **uncertifiable** (a proof obligation the analyzer could not resolve). Distinct from severity (error/warning).
_Avoid_: Finding status, finding type













**Discarded region**:
A source region the Solid compiler censused and then **deleted** — the `Value(Elided)` decision, projected as `ExecutionMap::discarded_regions` and classified `ExecutionRole::DiscardedRendering`. Distinct from an *untracked region*, which is code that executes once at render: a discarded region executes zero times, so it supports no finding and no certification. Silence over one means "both compilers deleted this", never "this was proven safe".
_Avoid_: Elided region, dead region, untracked region (that is the once-executing one), unreachable code

**Failure class**:
A user-facing grouping of the runtime misbehavior that findings prevent: silent staleness (reads that register no dependency), feedback loops (writes and actions in owned scopes), escaped async (pending reads outside tracked or Loading-bounded regions), and lifecycle leaks (effects, cleanups, and boundaries without a live owner).
_Avoid_: Bug category, rule group (that is the SCxxxx numbering)

**Source-edge refusal**:
A caller's recorded statement that source-boundary analysis was withheld for one exact occurrence of an import specifier (ADR 0310): an importer, the specifier as written, and a form plus the byte range of the whole statement or expression, held in the package boundary apart from package identities. The analysis joins it to the supplied facts by exact membership, keeps the package obligation open at that pair, and reports one neutral SC9005 uncertifiable finding per occurrence. It does not authenticate the caller's decision and establishes no missing module, missing export, runtime target or execution; a source implementation the program supplies is not withdrawn by it, and a record carries no generation, so the caller reacquires it for each facts generation.
_Avoid_: Source refusal, retained reason, missing-contract finding

**Dependency source**:
The exact installed runtime files selected for analysis alongside project source. Missing files or incomplete behavior remain unknown.
_Avoid_: trusted package, inferred contract

**Source selection**:
Resolution of runtime entrypoints for a declared target and build under the Vite and Solid plugin premises; distinct from proving behavior in those files.
_Avoid_: contract selection, name-based trust

**Package integrity premise**:
The condition that code outside the analyzed program neither supplies selected packages’ contexts nor rewrites their created objects (ADR 0300). Visible counterevidence withholds proof.
_Avoid_: authenticated package, proved integrity

**Unanalyzable import**:
An exact package import whose required runtime behavior could not be established from selected source. It remains an explicit SC9005 uncertifiable obligation.
_Avoid_: missing contract, safe external call
