# A conditional branch the compiler resolves to nothing, and the runtime loads

**The trap this fixture exists for: `index.js` must keep importing `#platform`
for its side effect only, and the `imports` map must keep offering more than one
branch.** Read a value from it and the analyzer refuses the entrypoint instead
(see the control below); leave one branch and this generation resolves it; make
the branches non-existent files and there is nothing a runtime could load.

`#platform` maps to `./platform-browser.mjs` under `browser` and
`./platform-node.mjs` under `node`. This generation selects neither condition, so
two things are true at once:

- The generator's syntax walk refuses to guess. Picking a branch would put a
  browser build's bytes behind a node build's summaries.
- The analyzing program resolves the specifier to **nothing**: `bundler`
  resolution selects neither branch, so the producer answers
  `resolution: "unresolved"` with an empty `resolvedPath`. The analysis read no
  file for it, and `index.js` alone is the complete and correct record.

**And that is exactly where saying nothing would be a wrong certification.**
Node, under its own conditions, loads `platform-node.mjs`; a bundler targeting
the browser loads `platform-browser.mjs`. Both are package code the analysis
never read, and a side-effect import of such a branch is where a package patches
a global, installs a polyfill, or calls into `solid-js/web` — precisely the
surface the contract's *negative* claims are about. So the claim that survives is
the same one a non-literal `import()` makes — the runtime may load a module the
analysis never read — and it rides the same field:

```
index.js: the module record is attested … and complete except for what #platform
may load at runtime: the analyzing program resolved nothing for it (…), while
platform-browser.mjs, platform-node.mjs exist on disk and a runtime selecting one
of them reads package bytes this analysis did not, which no module graph can
enumerate
```

`runtimeNotes`, not `notes`: the record *is* established, so two generations
whose records are byte-identical still transfer a review
(`closureDifference` in packages/cli/scripts/review-contract.mjs), while
`contract verify` still refuses the document
(`collectBlockers`, under the `attested-closure-note` blocker kind). Both halves
are pinned — the verify half in scripts/contract-verify.test.mjs, the transfer
half in scripts/contract-review.test.mjs.

**What separates this from `asset-import`, and why it is not an extension
guess.** `./styles.css` also reaches this branch: the walk fails on it and the
compiler resolves nothing for it either. The difference is a fact about files on
disk rather than a judgement about a file suffix — `runtimeTargets`
(`createModuleResolver` in packages/cli/scripts/runtime-module-closure.mjs) names
the existing *runtime modules* a runtime could still select for the specifier. An
asset import names none, and `./gone.js` names none, so no runtime resolves
either to a module and there is nothing left to say. Two conditional branches
that exist name two.

**The control, and why it is a test rather than a second fixture.** The
re-export form of the same package — `export { branch } from "#platform"` — never
reached this hole: the analyzer refuses the entrypoint outright, because the
re-exported binding's runtime kind no closed type answers. That refusal leaves
the package with no certifiable entrypoint at all, so generation exits non-zero
and there is no contract for a corpus fixture to pin. It is pinned in
scripts/contract-closure-record.test.mjs instead, which is what establishes that
the side-effect import was the *only* shape that could certify silently.
