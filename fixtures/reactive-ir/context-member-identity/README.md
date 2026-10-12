# Context member identity

Claim: an exact single-provider context member read is attributed to its
signal only through exact identifier identity.

- `App.tsx` / `tabs.ts` (positive): `TabsContext` and `useTabs` are published
  through a local export list in a closed program (`"private": true`). That is
  the same publication as `export const`: every importer is a censused
  reference. `TabsTrigger`'s helpers return comparisons over `ctx.active()`.
  A Type Facts entity demanded at such a composite span carries the symbol of
  its leading `ctx` token, not the boolean's value, so it must not be read as
  the shared context object and veto the context. `TabsList`'s untracked
  `ctx.orientation()` is SC1001.
- `Theme.tsx` / `theme.ts` (negative): a second provider rendered through the
  namespace member `themes.ThemeContext` has no exact identifier tag. It cannot
  be attributed to one context binding, so no provider census is closed and
  `ThemeLabel`'s read is not attributed to the signal. Without `FixedTheme` the
  same read is SC1001.

The open counterpart is `context-member-identity-open`.

Remaining approximation: a withheld context member is silent rather than an
SC9012 obligation (docs/precision-backlog.md).

`solid-js.d.ts` is the `accessor-provenance` stub plus `button.disabled`; the
claim-bearing signatures are copied from published 2.0.0-rc.13. The sources
pass `tsc --noEmit` (strict, skipLibCheck false) against installed rc.13.
