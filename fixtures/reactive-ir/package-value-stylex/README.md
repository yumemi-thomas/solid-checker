# package-value-stylex

**Claim (ADR 1063).** `@stylexjs/stylex` 0.19.0's runtime (both entries, `lib/es/stylex.mjs` and `lib/cjs/stylex.js`) defines `create`, `defineVars` and twelve more exports as functions whose whole body is `throw errorForFn(name)`. The reading is a premise (`stylex-0.19`, `solid-facts-backend/src/reviewed_packages.rs`) pinned to the digests of the manifest, both entries and `lib/es/stylex.d.ts`, and applied where the compiler resolves the callee to one of those declarations. A call of one returns no value, so a read of its result never runs: no `package-value-read` row at `Created` and `Tokens`.

Rows stay (uncertifiable SC1001, `package-value-read`): `Attrs` (`attrs` returns a fresh object, which the reading does not cover) and `OtherPackage` (another package's `create`). The twin `package-value-stylex-patched` changes one byte of `lib/es/stylex.mjs`: no premise, every row stands.

Package files: `@stylexjs/stylex@0.19.0` as published (npm, integrity `sha512-CnUFp7YMaDLDeemsWOfJgoC/gKM5P/yBNMcpJaE6ChJmXr7s0DJwSeGTTlHJcqqwN9OW1qGtmARWLFhGZN1pTA==`, from the rc.21 corpus's `inferay` install): `package.json`, `lib/es/stylex.mjs`, `lib/es/stylex.d.ts`, `lib/es/types/*.d.ts`, `lib/cjs/stylex.js`. `other-styles` is a stub. Solid stubs: `dependency-phantom-solid-plain`'s. `tsc --noEmit` clean.
