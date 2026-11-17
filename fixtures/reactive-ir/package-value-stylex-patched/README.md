# package-value-stylex-patched

**Claim (ADR 1063).** `package-value-stylex` with one byte appended to `node_modules/@stylexjs/stylex/lib/es/stylex.mjs` (a comment): the installed bytes are not the ones read, so the `stylex-0.19` premise does not hold and every `package-value-read` row stands (`Created`, `Tokens`, `Attrs`, `OtherPackage`).
