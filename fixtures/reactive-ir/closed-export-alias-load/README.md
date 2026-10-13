# A non-relative dynamic import names the module the compiler resolves

`helper` is entered by a visible direct call and named by a type query, which
alone would clear its caller-supplied-member obligation in this closed
application (ADR 0219). But `import("@entry")` loads it through a `paths`
alias, and `Object.values(module)` then enters it with an argument no call
site shows. The compiler resolves `@entry` to `helpers.ts`, so the obligation
stays (`SC9012`).

The compiler's resolution decides which module a non-relative specifier
loads. `kept` is entered by a direct call and named by a type query. The
program's dynamic imports resolve to `helpers.ts` and, for both `@other` and
`./other.js`, to `other.ts`; `require("fs")` names a Node built-in. None of
them exposes anything of `kept.ts`, so its obligation clears. Without
the attested resolution, a non-relative specifier may name any project
module, and `kept` would stay.

A Node built-in name loads the runtime's module, unless the compiler maps it
to a project file. Here `paths` maps `fs` onto `shimmed.ts`, so
`require("fs")` in `third.ts` may load it, and its namespace is enumerated:
`shimmed` stays (`SC9012`). The mapping adds that one file; `kept` still
clears.
