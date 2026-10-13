# ADR 0132: An installed package is attributed per export

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the missing-contract census
  (`rust/crates/solid-facts-backend/src/package_requirements.rs`,
  `is_package_local_edge`), and unknown-claim attribution
  (`rust/crates/solid-facts-backend/src/main.rs`, `runtime_binding_entity` and
  the `UnresolvedExportIndex` it feeds)
- Relation: completes the attribution ladder of `5fb13c25` for a package
  generated where it is installed. It adds no rung and widens no claim.
  `fallback-all` stays the answer wherever nothing exact ties an obligation to
  an export.

## Context

Every published package is generated inside `node_modules`. Many ship one
runtime module and one `.d.ts` per source file, with an entry that re-exports
them. `@tanstack/solid-router@2.0.0-rc.8` is one of them, with 97 root exports.
In the viviana-ui environment all 97 were published `{"call":{}}`: nothing was
determined, not even for `createFileRoute`, whose body calls nothing.

`unknown-claim-attribution` markers from the root node's generation (222 in
all) show two mechanics. Both depend only on the layout, and each one marks
every export.

1. **The package's own relative imports read as missing contracts.** The
   compiler reports a relative specifier as `NodeModules` whenever the file it
   lands on is under `node_modules`. That is true of every module of an
   installed package. `external_package_contract_requirements` therefore listed
   `import { useRouter } from "./useRouter.js"` as an import of a Solid-using
   package with no accepted contract. The obligation sits at the import
   declaration, outside every function, so attribution fell to `fallback-all`.
   The same bytes generated outside `node_modules` produce per-export
   summaries. Measured on a two-module reproduction: identical sources, a
   degenerate document under `node_modules/`, a precise one elsewhere. Only
   files with a bare import are attested, which is why the entry of a package
   with no external import was spared.
2. **Declaration identities do not join runtime functions.** The entry's
   `./useLocation.js` resolves to `useLocation.d.ts`, so each export's
   attribution identity was the declaration's. An obligation inside the
   runtime `useLocation` then joined no export. `export_names_for_function`
   answered "decided: not an export", and the ladder fell through to
   `fallback-all`. For an obligation inside a nested callback it instead
   answered `reachability` with *no* export. That was an under-attribution,
   masked only because a sibling obligation had marked everything. Deleting
   `useLocation.d.ts` alone turned the same obligations into
   `identity-widening` and `enclosing-chain` for `useLocation`.

## Decision

1. **A relative edge inside one package installation is not a package
   import.** The missing-contract census skips an import when both of these
   hold:
   - the specifier is relative (`./`, `../`);
   - the importer and the resolved file have the same owning manifest, which
     is the nearest `package.json` above each that declares a `name`.

   An unnamed nested manifest (`{"type":"module"}`) owns nothing. The marker
   stays for all of these:
   - a relative path that climbs into another installation;
   - a nested copy of the package;
   - an unresolved edge or an unreadable manifest.
2. **An export's attribution keys include its exact runtime binding.**
   `resolution.exports[name].runtime` is the resolver's module and export name
   for the public name, with that module's digest, and certification replays
   it from the archive. Attribution takes its entity by the same
   specifier-to-local walk it uses for the entry file, and adds that entity's
   runtime identity and canonical symbol beside the entry entity's. It does so
   only when both of these hold:
   - the module is an analyzed file under the package root;
   - the file's bytes hash to the recorded digest.

   A dependency's module, a sibling version, or changed bytes join nothing.
   There is no name matching.

## Soundness

(1) removes an obligation that could never be discharged. A package's own
module never has an accepted contract of its own. What it does is analyzed from
its body, in the same program, exactly as for the modules of a package that is
not under `node_modules`. That layout is the one every fixture already pins. An
edge that really leaves the package keeps the marker.

(2) makes an obligation attach to the export whose runtime function contains
it or reaches it, which is the answer the ladder already gives when no
declaration file intervenes. It is a narrowing only relative to `fallback-all`.
Relative to the join the ladder was designed around, it is a correction, and
it closes the `reachability`-to-nothing answer described above. Closures are
still decided by the certifier's census. Attribution decides only which
domains a proposal may carry into it.

## Consequences

- Pinned by `scripts/contract-installed-package-attribution.test.mjs`. It uses
  three packages, all installed under `node_modules` with a `.d.ts` beside each
  module:
  - the bundled control;
  - the unbundled package with an open dependency next door;
  - the same layout with nothing else.

  The unbundled `make` must match the layout control and keep the control's
  invoke claim. Both mechanics were falsified separately: with (1) disabled,
  `./fallback.js` goes `fallback-all`; with (2) disabled, `useOpaque`'s
  specifier obligation goes `fallback-all`. Either one alone fails the test,
  and so does the pre-change binary.
- On the solid-router root node, measured by replaying its graph-lane
  generation byte-identically, `fallback-all` obligations went from 78 to 44.
  The remaining 44 still mark every export, so the viviana run publishes
  exactly what it did before: 93 of 97 root exports degenerate. They are:
  - 31 import specifiers in the entry file for router-core and history names,
    each used only by the entry's own `export { … }` list;
  - references at module level (`class Route extends BaseRoute`,
    `class Router extends RouterCore`);
  - references in private helpers such as `getNotFound`, whose
    identity-widening does not continue through reachability.

  None of these is a join defect. Each needs a new rung.
- Not addressed here:
  - `creates` and `returns` are proposed only for functions in the entry file
    (a sibling-module `make` gets `callbacks` and `reads`, the bundled one all
    four).
  - The generator has written an empty `--runtime-module-resolutions` since
    `474c101f`. `declaration-sibling-reach`'s README describes the feed that
    existed before, and its obligation is `fallback-all` today.
