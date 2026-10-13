# ADR 0295: A declared Window realm decides `typeof window` in package source

- Status: accepted and implemented; opt-in as `--runtime-realm window`
  (2026-10-14).
- Owners:
  - the decision: `solid_facts::ast::HostConstantScope::in_window_realm`
    and `fold_host_constant_branches_in_window_realm`
    (`solid-facts/src/ast/host_constants.rs`);
  - the cycle proof: `dependency_sources::outside_import_cycles`, over
    `solid_facts::ast::module_load_specifiers`;
  - the flag and its scope: `fold_host_constants_in_realm` and the request
    parser in `main.rs`; `realm` in `scripts/coverage.mjs`'s `runtime.json`.
- Closes the parked `typeof window` entry of `docs/precision-backlog.md`.
  Research: `rust/target/research/typeof-window-premise.md`.

## Context

`@tanstack/solid-query` 6.0.0-rc.0 guards its server path with
`var isServer2 = typeof window === "undefined";` at module level and
`if (isServer2)` inside its functions, among them the `useBaseQuery` proxy
trap of ADR 0287. Nothing decided that guard, so under
`--dependency-source '*'` the trap kept its server branch and
spotify-desk-thing lost the two `prefer-show` findings contract mode reports
(`PlayerControls.tsx:133:5`, `PlayerControlIcon.tsx:26:5`).

`--runtime-target browser` does not decide it. Browser export conditions
also select worker, service-worker and SSR code, and a Node realm may have
DOM globals installed. A Window realm is a separate premise.

## Decision

1. **The premise is declared, never inferred.** `--runtime-realm window`
   (`"realm": "window"` in a fixture's `runtime.json`) states that the
   selected dependency-source modules run in a standards-conforming HTML
   Window realm. There, the global `window` and `document` exist and nothing
   replaces them. The flag needs `--runtime-target browser` and
   `--dependency-source`, and is refused without either. A target alone
   folds nothing new. Project files, inferred hosts, contract emission and
   certification are unchanged.
2. **The decided form is narrow.** It is a strict `===` or `!==` between the
   string literal `"undefined"` and `typeof` of the bare identifier `window`
   or `document`, in either operand order. Parentheses and type wrappers are
   stripped. The identifier must be an unresolved reference in Oxc's
   scoping: no parameter, local, import, catch, class or function binding,
   and no module-level declaration of that name. A module with a direct
   `eval` or a `with` decides nothing. `===` reads false and `!==` reads
   true, and `!` composes. Everything else stays live: `globalThis.window`,
   `window.document`, `typeof x === "object"`, `< "u"`, loose equality, a
   template literal, and any `&&`/`||` with an undecided operand. As with
   ADR 0166, only a whole `if` test that is decided with no side effect is
   replaced.
3. **A `const` alias carries it**, as an ADR 0166 alias does: never written,
   never redeclared. A read before the initializer throws, so it cannot take
   the other arm. (Review round 1: the throw is itself an effect the fold
   erased. A read now also needs an initialization proof.)
4. **A module-level `var` alias carries it only with an initialization
   proof.** A `var` reads `undefined` before its declaration runs, and a
   hoisted function can run that early. The alias must be one plain `var`
   statement with one identifier declarator, initialized with exactly the
   comparison, never written or redeclared. All of these must also hold:
   - every reference is a read, inside a function or textually after the
     declaration;
   - every module-level statement before it runs no code of the module.
     Allowed: imports, `export … from`, function declarations, classes with
     no heritage, decorators, computed keys, static blocks or static
     initializers, and declarations whose initializers are literals, `void`
     of a literal, function or arrow expressions, a Window-realm comparison,
     or a call of an imported binding whose arguments are literals or
     parameterless arrows returning one. That is exactly solid-query's
     layout (`createContext(null)`, `createContext(() => false)`);
   - the module is in no static import cycle. A module importing it back
     runs first and may call its functions. `outside_import_cycles` follows
     every static import, side-effect import and `export … from` through
     every package, as `prepare` resolves them (relative files, Node's lookup
     and `exports` for the host). It answers no cycle only when it can
     follow every edge. An unreadable or unparsable file, a `require`,
     `import x = require`, `import.meta.glob`, an uninstalled bare
     specifier, an unread `exports` form, a non-source target, or more than
     4,000 files all refuse. A dynamic `import()` is no edge: it settles
     after the importer's evaluation.

   The proof is asked only when such a `var` exists, once per module.

## Consequences

- spotify-desk-thing under `*` mode matches contract mode on every
  violation: 8, against 6 without the realm.
- The realm is stated by the caller. A worker, SSR or test-runner slice
  that selects browser conditions must not declare it. Nothing checks the
  declaration against the project.
- The `var` proof is stricter than truthiness needs for `=== "undefined"`:
  `undefined` and `false` are both falsy, so a pre-initialization read of
  `isServer2` takes the same arm. It is needed for the `!==` form, and one
  rule serves both.
- Not decided: `let` aliases, an exported `var`, `var` aliases declared
  beside another binding, any module-level statement before the alias that
  calls a module function or passes it a non-literal, and every module whose
  load closure reaches a CommonJS `require`.
- Contract emission and certification read no realm. Binding a realm into
  receipts is separate work (research, section 4).

## Evidence

- **Fixtures:**
  - `reactive-ir/dependency-source-window-realm`: three violations
    (solid-query's `var isServer2`, a `!==` alias negated, and a reversed
    `document` comparison) and five silent cases (a written alias, a
    `window` parameter, `globalThis.window`, a call before the alias, an
    import cycle). Each silent case reports once its one defect is undone
    (checked by hand on a copy).
  - `reactive-ir/dependency-source-window-undeclared` is the same project
    without the realm, byte-identical by `IDENTICAL_SOURCES`, and reports
    nothing.

  `tsc --noEmit` (5.9.3) is clean on both. Coverage otherwise unmoved.
- **Unit tests:** the grammar and its non-forms; parameter, destructured,
  default, local, catch, import, namespace, `var`, function and class
  bindings; direct `eval`; the solid-query layout; written, redeclared,
  `let`, exported, nested and multi-declarator aliases; a read before
  initialization; 16 kinds of statements before the alias; a cycle; the
  laziness of the cycle proof; load specifiers; and a cycle walk across
  packages, within a package, and through unfollowable edges.
- **spotify-desk-thing** (release binary, `--runtime-target browser
  --runtime-build development --rendering csr --dependency-source '*'`):
  with `--runtime-realm window`, +2 and nothing removed. The two new
  findings are `PlayerControls.tsx:133:5` (`nowPlayingQuery.data?.is_playing
  ? <A/> : <B/>`) and `PlayerControlIcon.tsx:26:5`
  (`props.showActiveIndicator && <span/>`, which `PlayerControls.tsx:122`
  feeds from `nowPlayingQuery.data?.shuffle_state`). Both read the query
  store through the client branch of the trap, which is the branch a Window
  realm runs. Both are contract mode's findings.
- **Corpus:** the same two modes over the 24 rc.13 corpus apps with a root
  tsconfig. The realm moves only those two findings. app-game exits 2 in
  both modes, on an Oxc parse error unrelated to this change.

## Review round 1

Four false-violation shapes, all reproduced as SC1001 on d9e50a946 and all
fixed:
- **A `const` alias read before its declaration.** The read throws
  (`ReferenceError`), and deciding it erased the throw. Decision 3 was
  wrong. A `const` alias now carries the realm only to a read proved to run
  after its initializer: in the same function and textually after it (not
  across `switch` cases), or in an arrow, function expression, method or
  class created after it. A read in a hoisted function needs the `var`
  alias's module proof. Any other read is decided as ADR 0166 decides it,
  without the realm.
- **A dead arm's `var` was deleted.** `if (typeof window === "undefined")
  { var Reflect; }` still declares the function's own `Reflect`. Replacing
  the arm with `;` made a later `Reflect.get` the global one. Now a dead arm,
  dead `else` or dead statement after a returning arm keeps its text when
  it declares a binding outside itself: a nested `var`, any function, or a
  class or other declaration beside the `if`. The condition is still
  rewritten. ADR 0166's fold uses the same rewrite, so it is fixed too.
- **An `exports` array hid a cycle.** `{"browser": ["./cycle.js"],
  "default": "./leaf.js"}` fell through to `default`. Vite reads it as
  `./cycle.js`. `select` now follows Vite's `resolve.exports`: the first set
  condition decides. An array, `null`, or a nested object that resolves
  nothing refuses. It does not fall through to a later key. The same selector
  picks dependency-source entries (ADRs 0284 and 0291), so such a package is
  now refused there too.
- **A package importing itself by name.** Inside `p`, `import "p"` resolves
  to `p`'s own `exports` (self-reference) before `node_modules`. The walk took
  a nested `node_modules/p` copy and saw no cycle.
  `runtime_package_root` now reads the nearest `package.json` first, as
  Vite's `tryNodeResolve` does. A manifest it cannot read refuses. Both the
  cycle walk and `prepare_all`'s bare-import queue use it.

The residual questions are also closed by refusing. The cycle proof refuses
a module that references `require` or `createRequire` under any binding, or
imports `module` / `node:module`. It also refuses any `import.meta` that is
not a static member other than `glob` (an alias, `import.meta["glob"]`).

Fixtures: `dependency-source-window-realm` gains `Review.tsx` with
`TdzQuery`, `HoistedQuery`, `ArrayCycleQuery` (packages `dep-array` and
`dep-bridge`) and `SelfQuery` (`dep-self` with a nested 0.9.0 copy). All four
are silent. Each one reports once its one defect is undone, checked by hand
on a copy. The undeclared twin carries the same bytes. Snapshots unmoved.
spotify-desk-thing (release binary, the flags above) is unchanged: 8
violations with the realm, 6 without, `PlayerControls.tsx:133:5` and
`PlayerControlIcon.tsx:26:5` kept.

Recorded, not fixed: outside a declared realm, an ADR 0166 `const` alias of
an imported host constant is still decided at a read in its temporal dead
zone. That fold mirrors the producer's `literalTruthinessLocked`, and
changing one side alone would split emission from certification.
