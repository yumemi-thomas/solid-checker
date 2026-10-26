# leaf-scope-project-callees

**Claim (ADR 0439; ADR 0209 unchanged).** A call in a leaf callback is followed into an import resolved by a relative path or a `paths` alias, a local function or `const` arrow, and a method declared in `C` of a `const` bound to `new C(...)`: a cleanup there is an `SC3001` violation, DOM work only is none. Else `SC9012`: a package with no source, a synchronous dispatcher (`click`) inline or in a helper, an inherited or twice-declared method, a parameter or element-access receiver, a reassigned `let` function. `solid-js.d.ts` copies `leaf-scope-exact-method`'s.
