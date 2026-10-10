# Review: does the rc.9-audited Solid 2 vocabulary hold for `2.0.0-rc.13`?

Date: 2026-10-05. Status: **review for the repository owner**. No dialect
answer, row, archive or pin was changed.

Owner request of 2026-10-05: focus on the latest Solid release. `solid-js`,
`@solidjs/signals` and `@solidjs/web` `2.0.0-rc.13` are `next` on npm,
published 2026-09-30. rc.12 was never published. rc.10 (2026-09-27) and rc.11
(2026-09-28) were not read. The template is the rc.3 → rc.9 review,
`2026-09-26-solid-2-rc9-vocabulary-review.md`, which led to ADR 0127.

The evidence is in three sections, each tagging every claim **[M]** (measured)
or **[E]** (estimated):

| Section | Covers | Method |
|---|---|---|
| [runtime.md](2026-10-05-solid-2-rc13/runtime.md) | timing, tracking, ownership, write guards, diagnostics (template §§ 2, 3.3, 3.4) | 134 probes on rc.9 and rc.13, dev and prod; function slices compared by digest |
| [surface.md](2026-10-05-solid-2-rc13/surface.md) | identity, packaging, `tsc` over the typings, export vocabulary, restated declarations, web entries, server bodies (template §§ 1, 2.1–2.3, 3.1, 3.2, 3.5, 3.6) | TypeScript API and `tsc`, runtime namespace listing, 15 probes |
| [compiler.md](2026-10-05-solid-2-rc13/compiler.md) | the compiler fork's runtime helper interface (template § 6) | helper list against every rc.13 web bundle, `npm view` |

Bytes: rc.9 is the audited archive
(`rust/target/audited-archives/solid-v2/2.0.0-rc.9`, matching
`AUDITED_ARCHIVES`). rc.13 was installed with npm into
`rust/target/audit-rc13`, with integrities:

- `solid-js` `sha512-62bYOI4JZ15KOqL5eReKyWSwAXrGb0fbX8SDnHsbJk2UX+SzSyy1gobas2cxW/0BXcobGfQ1V6ffPyQkMIBdoQ==`
- `@solidjs/signals` `sha512-4+pRdrAHtfyE9BUJWBup3TpzJojJjgW2mV1vm/Jik4tWa5epxXB/YrLkwqP1v8+S9XjyKKZu5BSLqmcpswMYeQ==`
- `@solidjs/web` `sha512-vI/7v/XM8B/3U/oKAzCW2VjTLFkM5IArB2DFnrjxyPRYD3FX9KAXg9QuAnPIPQHC/xvg/LgxmvoVfZUfJ96gOQ==`

## 0. Recommendation

**rc.13 is vocabulary-compatible with the rc.9 vocabulary. Move the audited
triple to rc.13.**

- Every timing, tracking, ownership and write-guard premise the rules rest on
  holds on rc.13's bytes, and every diagnostic the rule catalog cites is still
  emitted [M] (runtime § 3.3, § 3.4).
- The seven release-dependent answers the dialect gives rc.9 also hold on
  rc.13 [M] (runtime § 3.5, surface § 4): the store root typing, the static
  `dynamic` form, the `omit` predicate, `until`, the store setter root guard,
  `FLUSH_IN_ACTION` and the optimistic-store setter guard.
- rc.9's five broken root re-exports (N2) are fixed: the surface type-checks
  with 0 errors under `skipLibCheck: false` [M] (surface § 2.3).

What does not hold is bookkeeping. The release table reads only up to rc.9
(`releases.rs`, `NEWEST_READ = 9`). An rc.13 install is therefore analyzed
today under the **conservative** vocabulary and loses all seven answers [M].

The compiler fork's runtime helper interface is compatible with rc.13, as it
was with rc.9 [M] (compiler § 1–2). A rebase candidate onto the rc.13 compiler
exists, but adopting it needs the fork branch pushed (§ 4).

## 1. Items that need a change

| # | Item | Evidence | Change |
|---|---|---|---|
| D1 | The release table does not know rc.13. Bumping `NEWEST_READ` would be wrong: three answers test equality with it, and rc.10–rc.12 would read as reviewed. | surface § 4 B1 [M] | An explicit set of read releases, {rc.0–rc.9, rc.13}, with rc.13 answering as rc.9. `AUDITED_INSTALLATION` names the rc.13 triple. rc.10–rc.12 stay unread (conservative, under `SC9014`). |
| D2 | `AUDITED_ARCHIVES` has no rc.13 tuple, so no archive-scoped negative row answers for rc.13. `untrack` and `createOwner` changed in content; the other rows of the rc.9 "five creates" set are identical or minifier renames. | surface § 4 B2 [M] | Add the rc.13 archives. Re-read each carried row on rc.13 bytes, as ADR 0127 did for rc.9; re-derive the integrities from the tarballs. |
| D3 | `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`, `sharedConfig` and `$DEVCOMP` are declared only in `solid-js/internal` on rc.13. The dialect's export table maps them to the root, and `modules()` does not own `solid-js/internal`, so findings on them are lost (not wrong). | surface § 4 B3, runtime row 8 [M] | The dialect owns `solid-js/internal` for these names. A root import of them is TS2305 on rc.13, and the checker stays silent there. |
| D4 | New dev warning `UNTRACKED_READ_AFTER_AWAIT`, the runtime counterpart of SC1002 `reactive-read-after-await`. It is silent for a source already read before the `await`, inside `untrack`, in a plain async function, in an async effect apply or tracked effect, and in an action body. | runtime row 1, § 3.4.2 [M] | Cite it on the rule page. **Check that SC1002 agrees with each silent case**: one where SC1002 reports and the runtime does not is a precision question to settle before rc.13 becomes the measurement baseline. |
| D5 | `lazy`-loaded component bodies run in a strict-read window under a root on rc.13 (`createComponent`); rc.9 ran them as `untrack(() => Comp(props))`. | runtime row 2 [M] | None for rc.13: it now matches what SC1001 assumes for every component. Record that the assumption held for lazy components on rc.9 only from rc.13 onward. |

## 2. Recorded, no change

- `<Errored>` calls any function fallback, including a zero-parameter one
  [M]. No dialect row models the fallback; a future one must be
  release-aware.
- Cleanup order reversed: disposal now runs last-registered first, dev and
  prod [M]. No rule or contract states an order.
- `<Loading on>` became a tracked dependency list [M] (surface).
- New guards no rule mirrors (rule opportunities): `LOADING_ON_OUTSIDE_HOLD`
  (dev warning) [M], and `DYNAMIC_ASYNC_COMPONENT` (a server throw on every
  tier, next to SC2007) [M by source].
- 15 diagnostic codes were added and 4 removed or renamed [M]. The 12 codes
  the catalog cites are all present.
- The callback-taking-export completeness test reads the bundled rc.3
  contracts and cannot fail on rc.13 today. If those are regenerated,
  `armLiveBody`, `ssrSanitizeError`, `setConsoleFooter` and `ssrElement`'s
  `attrs` need a modelled-or-excluded decision (surface § 4 B4).

## 3. Not measured

- rc.10, rc.11 and rc.12: not read, so they stay unread.
- The rc.13 integrities are the lockfile's records; no tarball was
  re-derived.
- No `tsc` run for the runtime section, no checker run on rc.13 code, and no
  SSR, hydration or observe-tier execution.
- The `#3648` (`isPending`) and `#3621` (self-disposal) fixes were not
  reproduced by any probe.

## 4. Compiler

The checker compiles with the semantic-facts fork pinned at `9f9a84b2`, an
rc.3-era compiler [M]. Every helper it can emit is still exported by rc.13
`@solidjs/web` in all six bundles [M], so rc.13 can be audited on the current
pin, as rc.9 was.

Since `@solidjs/vite-plugin` next.34 (2026-08-26), consumers compile with the
native `@solidjs/compiler`. Its rc.13 build emits two helpers the pinned fork
does not know (`readShallow`, `ssrElementAttribute`); both are rc.13 runtime
exports [M].

A rebase of the fork onto rc.13 was prepared and validated on 2026-10-03
(`phase22/2026-10-03-rc13-compiler-facts-rebase.md`, candidate `3ad4bbe`).
Adopting it starts with making that branch available to Cargo, which means
pushing it. That is the owner's call, and it is independent of moving the
runtime triple.

## 5. Proposed order

1. D1, D3 and the `AUDITED_INSTALLATION` move, with D2's archives and rows,
   as one ADR (the rc.13 counterpart of ADR 0127).
2. D4: settle SC1002 against the runtime's silent cases.
3. Move the tsc oracle, archive provisioning, runtime pins and benchmark
   ceiling to rc.13, each in its own commit, as for rc.9.
4. Build an rc.13 copy of the 38-app corpus as the measurement baseline.
5. Compiler adoption, once the owner decides on pushing the fork.
