# A contract with no classified install may not reach project source

A package contract is applied to an import only when the resolution the
compiler recorded confirms the install the contract was classified against.
When classification found no installed directory — the package is not under any
`node_modules` above the project — there is no directory to compare, and the
only facts left are the two package *names* the resolution recorded. Names are
not enough, and this fixture is the case that proves it.

- `selfNamedByPaths` imports `"reactive-package"`. Nothing named
  `reactive-package` is installed; the contract comes from
  `.solid-checker/contracts/reactive-package/`, so it was classified against no
  install. `paths` maps the specifier to `src/local-impl.ts`, and the nearest
  `package.json` above that file is this project's own — which declares
  `"name": "reactive-package"`. The names therefore **agree**, and the contract
  is still **refused**: the compiler reports a resolution that landed outside
  every `node_modules` tree, and a bare specifier resolving there is a `paths`
  or `baseUrl` mapping, a package self-name, or a project-reference redirect
  (the compiler does not record which). All three are source this project owns.
  No `SC9005` obligation is raised, and `src/local-impl.ts` is analyzed on its
  own terms.
- `uninstalledWithNoResolution` imports `"uninstalled-package"`, which is
  genuinely not installed, carries a project-owned contract, and is typed by an
  ambient `declare module`. The compiler resolves the specifier to nothing.
  Nothing resolved means nothing else claimed it, so the contract **applies**
  and raises its one `SC9005 package-contract-incomplete` obligation for a
  callback passed by name. This is the control that keeps the fix honest: a
  contract for an uninstalled package still works. What stops working is a
  contract reaching the analyzed project's own source.

This is the shadow scenario of
`fixtures/reactive-ir/package-contract-paths-shadow` with the install removed,
which moves it from the containment clause to the name-equality clause. Against
a pre-change binary — and against the first version of the identity gate — the
contract's callback claim was raised at a call whose callee is
`src/local-impl.ts`, a file no reviewer of the published package ever saw. Both
directions live in one project on purpose: if binding broke entirely the
control's finding disappears, and if it degraded to name equality a second
finding appears.

The refusal is deliberately silent — it produces no finding of its own, exactly
as an import of a package with no contract produces none. It is counted, though:
`SOLID_CHECKER_TIMINGS=1` reports `contractBindingsRefused: 1` and
`contractBindingsBound: 1` for this project, so the refusal is quiet rather than
invisible.

`tsc --noEmit` is silent here. `src/local-impl.ts` declares the signature the
call needs, the ambient declaration satisfies the other import, and `named`
matches both. Which package a specifier resolves to, and which contract may
therefore describe it, is a resolution and provenance question the type system
does not model.

There is no `node_modules` in this fixture at all, which is the point: it is the
only shape in the corpus where the contract loader has no installed directory to
compare against.
