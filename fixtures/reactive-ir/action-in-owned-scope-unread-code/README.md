# action-in-owned-scope-unread-code

**Claim (ADR 1002).** Code of a package that nobody read may write any global. ADR 0974 records the globals a package's files write; a closure with code the
walk did not read (a load by a computed name, a load no installed file answers, a file the parser refuses, a native module, a manifest the walk does not read,
files past the closure limit) was a writer only of what its read files wrote, so a global written by the unread part had no writer and its call was silent.

- `fz-late`'s entry loads `./late.js` by a computed name (`dynamic-load`). `late.js` replaces `btoa` with a function that calls a Solid action.
- `Plain`, `Other`, `Member`: a call of a standard-library global in a component body is the uncertifiable SC2002 row a call of a global some package writes
  under any name has (`package-may-call-action`), whichever global it names.
- `Shadowed`: a local of the same name is no global: silent.
- The other side, a package read to its end that writes one global: `action-in-owned-scope-global-spellings` (`Unwritten` is silent there).

`node_modules/fz-late` is **not** published bytes: it is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of
`action-in-owned-scope-unread-packages`; `.solid-checker/runtime.json` closes the program boundary.

Chrome: the pattern lane's `pkgglobal/g06-computed-import` (the same package; the runtime oracle pre-bundles a case's packages, which moves the file the
computed name loads, so the case runs in the pattern lane, which serves each file as it is).
