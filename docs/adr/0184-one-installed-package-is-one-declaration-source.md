# ADR 0184: One installed package is one declaration source

- Status: accepted and implemented (2026-10-05). Eighth lever of the owner's
  package-misuse goal of 2026-10-04; the first aimed at real app installs
  rather than at the checkpoint tier.
- Owners:
  - the Type Facts side's source filter
    (`retain_collision_free_source_packages_with_reasons`);
  - the CLI's declaration-source collector (`createCompilerSourceCollector`
    in `packages/cli/scripts/certify-contract.mjs`);
  - the refusal wording of `plan_contract_document_with_sources`.
- Relation: a package certified in place (the `SC9005` remedy) is admitted only
  with a dependency environment. These changes let pnpm installs acquire one.
  No wire change and no new trust: every source still authenticates against
  its exact lock selection.

## Context

Certifying each 38-app project's installed packages in place gave 90 of 180
certifications. Several `@solid-primitives` packages certified but were then
refused admission:

> certified but not admitted: dependency environment not acquired: declaration
> source package(s) solid-js did not authenticate against their lock selection

The message named neither the step nor the reason. With the reasons recorded,
two defects appeared, both specific to pnpm's layout.

1. **A false collision.** One `solid-js` reached through two install paths
   becomes two requests, and therefore two identities: pnpm's
   `node_modules/solid-js` link, and its `.pnpm/solid-js@…/node_modules/solid-js`
   target. A source's identity includes its installed root. Both land on the
   private project's `node_modules/solid-js`, so the collision filter withheld
   both.
2. **A walk from the link.** The collector looked up a source's own
   dependencies by walking up from the importer's path as written. From the
   link (`app/node_modules/solid-js/…`) no `@solidjs/signals` is installed
   above. From the target, it sits beside `solid-js` under `.pnpm`, which is
   where Node and TypeScript (without `preserveSymlinks`) resolve it.

## Decision

1. **Same bytes are one source.** Two authenticated sources on one
   private-project root that agree in name, version, integrity, snapshot root
   and provenance root are kept once. The copy kept is the canonical one, and
   it carries both requests' resolution edges. Sources that differ in content
   still collide and are withheld, as before.
2. **A dependency nothing is installed above is looked up from the importer's
   real path.** The written path is tried first, so every lookup that
   succeeded before is unchanged; this matters on macOS, where a temporary
   directory's real path moves `/var` to `/private/var` and lock lookups key on
   the written one. Only when it reports `package-not-found`, and the real path
   differs, is the lookup repeated from the real path.
3. **A refusal says why.** Each dropped name carries its reason: the planning
   error of its request, or which collision withheld it.

## Consequences

- No source is admitted that was not before, unless it is the same
  authenticated bytes reached a second way.
- `preserveSymlinks` is not read. A project that sets it resolves from the
  link, and the collector does not follow that.
- A written path that finds a hoisted copy above a link keeps it, as before,
  even where Node would resolve the sibling beside the link's target. That
  pre-existing approximation is unchanged; the copy found must still
  authenticate against its exact lock selection.

## Evidence

- Rust test `one_source_package_through_two_install_paths_is_kept_once`: two
  install paths of the same bytes are kept once. The same name with other bytes
  still collides, and the reason is recorded.
- CLI test `a pnpm-linked source locates its own dependencies beside its
  target`. Without the real path it fails with the error the apps produced:
  `gamma is not installed above …/node_modules/alpha/types/index.d.ts`.
- 38-app local certification rerun (`rust/target/local-certify/local-0184`):
  - certifications go from 90 to 100 of 180. `app-game` alone goes from 31 to
    36 of 37: `@solid-primitives/deep`, `keyed`, `map`, `set` and
    `transition-group` are admitted;
  - one more project has a before/after pair, 39 instead of 38;
  - violations go from 516 to 525: +9, −1. The +9 are the eight earlier
    TanStack router reads, plus one new one in `spotify-desk-thing`: a
    `useMutation` store member read in a `createEffect` apply. On the
    installed `@solidjs/signals@2.0.0-rc.4`, the effect function runs with
    the strict-read label "an effect callback" and no observer, and the store
    `get` trap warns for any such read. So it is `STRICT_READ_UNTRACKED`, a
    true positive. The −1 is the known `combineProps` partial-contract
    precision loss (`overlays`).
- Remaining admission walls, with reasons now named:
  - `solid-js/store` is not exported for a legacy renderer case (11);
  - the `@tsrx/core` environment (8);
  - emit refusals (6);
  - witness acquisition (3);
  - lock selections Bun does not record (`csstype`).
