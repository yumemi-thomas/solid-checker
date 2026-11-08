# primitive-namespace-aliases

**Claim (ADR 0901, review D36).** A binding that exactly holds a Solid primitive names it, so its call is that primitive's for every rule, as the direct call is. Exact: a namespace member (`const r = S.refresh`, `S["refresh"]`), a destructured slot whose default is the same member (`const { refresh: r = S.refresh } = S`), a non-exported `const` of the named import (`const r = refresh`), and a closed holder's member (`const box = { m: refresh }; const r = box.m`) when the holder is only read by `const` initializers and the file has no direct `eval`. `Exact` and the first `createRoot` give each alias the rows `Direct` gives `refresh(m1)` and `flush()`.

Negative (`NotExact`, the second root): a written `let`, a default naming another value, a holder that escapes or is written, a plain object that is no namespace, an exported alias. Each names no primitive: SC9012 `package-callee` at the call, never a primitive's violation (ADR 0884 adds its rows where the compiler resolves the call to a primitive, and ADR 0902 where the operand's `const` initializer names one: the exported alias gets SC9012 `primitive-alias-call`, still no primitive rule's row).

Variants:
- `eval-box`: a direct `eval` can write the holder unseen; the boxed member names nothing.
- `tdz`: a call that runs before the alias's declarator throws `ReferenceError`; the ADR 0339 guard withholds the name, so it gets only SC9012 (`primitive-value`, and ADR 0902's `primitive-alias-call`), no `refresh` rule row.
