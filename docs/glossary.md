# Canonical glossary

Use these terms in code, diagnostics, contracts, and design discussions.

| Term | Meaning |
| --- | --- |
| Reactive provenance | Accepted proof that a value or property is reactive and the binding flow by which it reached an operation. |
| Execution region | Original-source code executed with a compiler/runtime-defined role such as tracked, untracked, owned, event-driven, deferred, cleanup, effect-compute, or effect-apply. |
| Reactive operation | A read, write, call, callback invocation, primitive creation, cleanup registration, async read, or ownership operation in the Reactive IR. |
| Effect summary | An interprocedural description of a function's reactive behavior. |
| Proof obligation | A fact that must be established before a project can be certified. |
| Package contract | A static description of the reactive effects of a package's exported interface. |
| Claim domain | One independently knowable set of package behavior. A claim domain can remain open while sibling domains are closed. |
| Domain closure | Accepted proof that one claim domain exhausts the possible behavior for its exact subject and scope. |
| Contract proposal | Unaccepted machine output containing behavioral candidates and proof obligations. |
| Accepted package contract | A normalized contract whose closed claims and exact identity are bound by a valid acceptance receipt. |
| Artifact case | One exact entrypoint-resolution result with its runtime artifact, declarations, and dependency closure. |
| Operation graph | A causal model of semantic operations and their scheduling, data, cleanup, error, and lifetime relationships. |
| Evidence sidecar | A hash-bound artifact containing detailed proof inputs and runtime observations, not required during ordinary analysis after acceptance. |
| Acceptance receipt | A verifier-issued binding between contract meaning, exact artifacts, proof material, and verification policy. |
| Semantic digest | The canonical identity of normalized contract meaning, independent of wire formatting and compression. |
| Compiler semantic trace | The compiler-owned transform-local record of semantic decisions made while lowering one source file; it observes rather than changes lowering. |
| Compiler execution facts | The checker-normalized fact domain projected from a validated compiler semantic trace, with an independent protocol version. |
| Violation | A proven breach of a Solid reactive rule. |
| Uncertifiable | An unresolved proof obligation at an explicitly unsupported or unavailable boundary. |
| Certification snapshot | A deterministic project result containing status, findings, explanations, fixes, package summaries, and metrics. |
| Demand closure | The transitive fact set one analysis generation needs, computed by the type-facts service from seeds; never the full universe. |
| Closure request | The TypeFacts v2 request: seeds plus a pinned expansion-ruleset version; answered by one closed fact table per generation. |
| Expansion ruleset | The versioned rules that grow seeds into a demand closure; changing them is a wire-visible change and reruns the affected gates. |
| Generation | One accepted analysis version of a type-facts project. Every answer given within a generation is mutually consistent; an accepted update starts the next generation. |
| Edit exchange | The single per-edit exchange with the type-facts service: the update that advances the generation always lands, and the analysis answering the new generation may be cancelled independently. Cancelling an edit exchange never cancels its update. |
| Affected set | The files whose answers an accepted update may have changed. Single-file external-module edits stop at the edited file when canonical declaration shape, resolved imports, and exported durable IDs are unchanged; otherwise the set is the changed files plus their transitive importers. |
| Durable symbol identity | Symbol identity derived from the declaration itself (declaring file, declaration location, name), so it stays meaningful across generations while that declaration is unchanged. |
| Source-fact memo | The cross-generation cache of per-file source facts. Entries outside an update's affected set carry over; entries inside it are recomputed. |
| Retained closure contribution | One file's share of the demand closure, carried across generations while the file stays outside every accepted update's affected set, its demand run is unchanged, and every identity it carries is durable. |

## Naming constraints

- Do not call a heuristic a proof.
- Do not call an unresolved obligation safe.
- Do not expose TypeScript, Oxc, or solver-specific node terminology through
  the certification package.
- Use “adapter” for CLI and LSP integrations; neither owns reactive
  analysis.
