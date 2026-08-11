# tsgolint extraction and removal plan

The module this plan governs lives in the
[solid-ts-facts](https://github.com/yumemi-thomas/solid-ts-facts) repository;
the `internal/typefacts` and `go.mod` paths below are relative to that
repository, and this checker consumes the result as a pinned dependency.

Milestone 1 uses tsgolint only as a reviewed bridge to unstable
typescript-go APIs. The checker does not embed tsgolint's product, rule, CLI,
configuration, diagnostic, or LSP layers.

## Retained dependency surface

`internal/typefacts/tsgo` imports only the pinned shim modules needed to:

- parse a `tsconfig.json` and construct a TypeScript program;
- bind source files and obtain a checker;
- resolve source nodes internally from original UTF-8 byte locations;
- query symbols, aliases, declarations, types, signatures, and modules;
- maintain an in-memory filesystem overlay.

No shim, TypeScript AST node, checker type, symbol, program, or scope object is
part of the public `typefacts.Project` contract.

## Modules intentionally excluded

The extraction must never acquire dependencies on tsgolint's:

- CLI or configuration loading;
- lint rule registry or rule implementations;
- diagnostic formatting and suppression;
- JavaScript/TypeScript AST wrapper exposed to rules;
- worker orchestration or product-specific incremental scheduler;
- editor or language-server integration.

An import audit in CI should reject tsgolint packages outside the shim modules
pinned together in `go.mod`.

## Removal sequence

1. Keep the contract and integration fixtures in `internal/typefacts` as the
   compatibility boundary.
2. Replace shim calls behind `internal/typefacts/tsgo` with direct official
   typescript-go APIs when they provide equivalent bulk access.
3. Evaluate the official TypeScript 7.1 programmatic API against the same
   alias, type, signature, reference, project-reference, and update fixtures.
4. Remove each corresponding shim replacement from `go.mod` only after the
   official adapter passes correctness and performance comparisons.
5. Retain the tsgolint-derived adapter if the official API cannot provide the
   required facts without per-node IPC or material loss of incremental
   performance.

This is a dependency-removal plan, not a deadline tied to TypeScript 7.1. The
pinned adapter remains supported until evidence justifies replacement.
