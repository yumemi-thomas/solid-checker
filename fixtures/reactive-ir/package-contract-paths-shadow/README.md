# Package contracts are bound to an installed package, not to a specifier name

A package contract describes one installed package. Contract discovery and
contract application both used to key on the import specifier's package root
and nothing else, so a tsconfig `paths` entry that maps a bare specifier onto
project source while a package of that name is still installed got the
installed package's contract applied to code the contract never described — a
false certification, not a missed one. This fixture pins both directions of the
fix in one file, so a regression cannot be mistaken for the fixture failing to
load.

- `shadowedByPaths` imports `"reactive-package"`, which `paths` maps to
  `src/local-impl.ts`. The compiler reports the resolution as `nonRelative`
  landing outside every `node_modules` tree, so the resolved file is not inside
  the installed directory the contract was classified against, and the contract
  is **refused** for this specifier. The call is then exactly as certifiable as
  any project-source call: `src/local-impl.ts` is analyzed on its own terms, and
  the contract's `SC9005` obligation is *not* raised, because no contract
  applies.
- `installedIdentity` imports `"other-reactive-package"`, an ordinary install
  with no `paths` entry and an identical contract. Its specifier resolves inside
  `node_modules/other-reactive-package`, so the contract binds and raises the
  one `SC9005 package-contract-incomplete` obligation for a callback passed by
  name — byte-for-byte the behavior of
  `fixtures/reactive-ir/package-callback-arguments-consumer`.

The refusal is deliberately silent: it produces no finding of its own, and the
import becomes uncertifiable exactly as an import of a package with no contract
would. A refusal that announced itself would be a claim about the project's
configuration, which is not this checker's subject.

`tsc --noEmit` is silent on this project: `src/local-impl.ts` declares the same
signatures the installed declarations do, and `named` matches them. The
refusal is a fact about *which package* a specifier resolves to, which the type
system has nothing to say about — it type-checks the local implementation and
is satisfied.

The declarations in both installed packages are exact for their package; every
finding depends on the runtime contract, not on trusting a declaration as
runtime evidence.
