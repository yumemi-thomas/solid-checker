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

**Configured source descriptor**:
A Type Facts session source entry that names a canonical disk path for local hydration, while edited or virtual sources remain inline. Generation source hashes still prove that Rust and TypeScript-Go analyzed identical content.
_Avoid_: Path-only source, source shortcut

**Semantic lookup**:
The project-wide query surface rule discovery asks for semantic answers — the entity or symbol at or containing a location, the function a symbol names, whether an owner is rendered under a Loading boundary — instead of scanning fact tables.
_Avoid_: Index helpers, fact-table scan, range-query module

**Fact domain**:
One of the independent evidence suppliers the checker cross-references: Oxc syntax facts, Solid compiler execution facts, Type Facts, and package contracts. User-facing documentation may call them "sources of evidence"; the canonical term is fact domain.
_Avoid_: Backends, sidecars, analysis inputs

**Rule options**:
The project-level per-rule configuration document, `.solid-checker/rule-options.json`, discovered beside a project's contracts and carrying the upstream eslint-plugin-solid options the 1.x rules honour. Defaults are upstream's defaults; parsing fails closed. Part of every build and diagnostic identity.
_Avoid_: Rule config, checker settings, options file

**Finding kind**:
Whether a finding is a **violation** (the analyzer proved the code misbehaves at runtime) or **uncertifiable** (a proof obligation the analyzer could not resolve). Distinct from severity (error/warning).
_Avoid_: Finding status, finding type

**Failure class**:
A user-facing grouping of the runtime misbehavior that findings prevent: silent staleness (reads that register no dependency), feedback loops (writes and actions in owned scopes), escaped async (pending reads outside tracked or Loading-bounded regions), and lifecycle leaks (effects, cleanups, and boundaries without a live owner).
_Avoid_: Bug category, rule group (that is the SCxxxx numbering)
