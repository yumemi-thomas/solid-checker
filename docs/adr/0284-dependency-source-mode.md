# ADR 0284: Dependency source mode

- Status: accepted, experimental and opt-in (2026-10-13).
- Owner: `solid-facts-backend::dependency_sources` (selection, closure,
  program), `main.rs` (redirects, host folding, contract removal), and the
  ADR 0284 filters in `diagnostics.rs`, `package_requirements.rs`,
  `runtime_configuration.rs` and `accessor_provenance.rs`.

## Context

A dependency's behavior reaches the analysis only through an accepted package
contract. Contracts are authored or certified per package and per version,
and most installed packages have none. Without one, every import of the
package is an SC9005/SC9011 obligation, and every read flowing through it is
silent. The package's runtime source is installed next to the project, and
the analysis already proves project code.

## Decision

`--dependency-source <specifier>` (repeatable; `dependencySources` in a
fixture's `.solid-checker/runtime.json`) analyzes the runtime source of an
installed package as project code.

1. **Selection.** The package's `exports` are read the way a Vite +
   `vite-plugin-solid` build for the declared host reads them. The first key
   in document order wins among `solid`, `import`, `module`, `default`, and the
   host's own conditions: its target (`browser` or `node`), its build
   (`development` or `production`) and every `--runtime-condition`.
   - **No `exports`:** Vite's `mainFields` decide: `browser` (string form) for
     a browser host, then `module`, then `main`. A per-file `browser` map is
     refused.
   - **Host required:** the mode refuses to run without a declared
     `--runtime-target` and `--runtime-build`, because the file a package
     loads depends on both.
   - **Other refusals:** a pattern (`./*`), an array, or no selectable target
     is an error naming the package. A package the project does not install is
     skipped with a note. Nothing guesses a file.
2. **Closure.** Relative static imports and re-exports are walked from the
   entry, inside the package, to at most 4,000 files. A relative import that
   resolves to no source file is an error.
3. **Program.** A sibling tsconfig extends the project's, adds `allowJs`
   (`checkJs: false`), and lists the closure as root files. It is removed when
   the run ends. The Type Facts daemon is bypassed.
4. **Binding.** The compiler still resolves the package to its declarations.
   Each import of the package by its bare specifier, and each relative import
   inside it that resolves to declarations, is redirected to the export entity
   of the runtime file it loads (the ADR 0186 `runtime_symbol_redirects`
   mechanism). A source bound to two targets binds none.
5. **Host.** Package files are folded under the declared runtime host
   (`selected_conditions()`), like project files.
6. **Contracts.** The selected packages' accepted contracts are not bound, and
   their imports no longer require one.
7. **Reporting.** A finding located in a package file is not reported. Its
   effect on project values is reported at the project's call sites.
8. **Premises.** Dependency internals stay outside the project's premises,
   as when only declarations are seen:
   - a package's own `DEV`, `OBSERVE` or hydration-host use vetoes no
     project finding (standard runtime configuration premise);
   - a write inside a package that could name any member (ADR 0273's
     wildcard veto, such as `Object.assign` with an open source) vetoes only
     members of objects that same package creates. A write naming its member
     vetoes that member everywhere, because the project may have handed the
     package the object.

## Consequences

- A package with no contract can be analyzed. Removing a contract does not
  lose its findings when the source proves them.
- Recall still depends on what native analysis proves about package code,
  which is often compiled JavaScript with `any`-typed parameters. ADR 0285
  (forwarded parameters) came out of this mode.
- Known losses against contract mode:
  - a `new Proxy(store, { get })` result (solid-query `useQuery`);
  - tuple slots returned by a hook (`@solid-primitives/raf`; ADR 0282 covers
    destructured closures);
  - a context-provided value read through a package hook (kui's location);
  - a finding a package call causes but that is located in the package (an
    `onCleanup` without an owner reached from `createTimer`). It is dropped,
    not attributed to the project's call.
- `./*` export patterns (Kobalte) are refused.
- A summary-propagated read inside an effect apply is labelled "rendering
  function" rather than "apply callback". This also happens in base mode, so
  it is wording, not role.

## Review round 1

An adversarial review found two false violations, both fixed:
- a fixed browser/development condition set selected the source a Node or
  production host never loads;
- without `exports`, `module` was read before `browser`.

It also found that scoping named writes per package discarded a visible
write on a project object. A project factory that hands its object to a
package that replaces a member stayed a violation. Named writes are now
global. The same shape in project code alone
(`const tag = { label }; relabel(tag); tag.label()`) is a false violation
that does not depend on this mode. It is recorded in docs/precision-backlog.md
and is being fixed separately.

## Evidence

- **Fixture** `reactive-ir/dependency-source`: the `solid` condition is
  preferred over `default`, a relative closure is followed, a package-located
  finding is dropped, and a package `Object.assign` does not veto a project
  context. Without the flag, the same sources report SC9011 at both hook calls.
- **error-menu-web:** with `@solidjs/router` selected, two `load` reads
  vanished. The router's `Object.assign(result || { ...raw }, outcome.value)`
  vetoed every transported member project-wide. Scoping the veto (decision 8)
  restored both; the only remaining difference is the SC9005 notice that the
  mode removes.
- **rc.13 corpus**, 11 packages (`@solidjs/router`, `@solidjs/meta`,
  `@tanstack/solid-router`, `@tanstack/solid-query` and seven
  `@solid-primitives`), release binary, browser host:
  - 0 errors;
  - violations 562 → 559, uncertifiable 3,731 → 3,557;
  - 3 added violations, all probus-hk router accessor reads (`createSignal(urlQuery())` in a
    component body, and two reads in effect apply callbacks), the same class
    base mode reports from contracts;
  - the 6 lost violations are the losses listed above;
  - no false positive found.
