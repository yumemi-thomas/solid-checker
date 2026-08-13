# refresh-target-unresolved

`SC9003` · **error** · uncertifiable

The target passed to `refresh()` cannot be traced back to a Solid source.

## What it does

Flags `refresh(target)` calls where the target identifier cannot be resolved to a
binding the analyzer knows to be a branded Solid source (a derived signal, store,
or projection) — typically values that crossed a file or package boundary the
analysis cannot see through.

## Shared code

SC9003 identifies the uncertifiable `refresh()`/`affects()` target-provenance
family; the rule name identifies which API surface reported it. A suppression
or filter by code therefore silences both `refresh-target-unresolved` and
`affects-target-unresolved`. Project-wide enablement uses exact rule names, so
either surface can be disabled without disabling the other.

## Why is this analysis-limiting?

`refresh()` requires a branded source; an unbranded value throws at runtime
(see [invalid-refresh-target](invalid-refresh-target.md)). When the analyzer cannot
trace where the target came from, it can neither certify the call nor prove it
wrong — the finding is uncertifiable until the provenance is visible.

## Examples

Code this rule flags:

```tsx
import { currentUser } from "some-package"; // No contract entry describing what this is.

refresh(currentUser); // Source? Wrapper? The analyzer cannot tell.
```

Code that resolves:

```tsx
// In-project sources are traced directly:
const user = createMemo(() => fetchUser(id()));
refresh(user);
```

## How to fix

Pass the binding created by `createSignal`, `createMemo`, `createStore`, or
`createProjection` directly. If the source is re-exported or wrapped by a package,
declare that export's return kind in the package's reactivity contract so the brand
survives the import — see [package-contracts.md](../package-contracts.md).

## Related

- [invalid-refresh-target](invalid-refresh-target.md) — the proven-invalid variant
- [package-contract-export-missing](package-contract-export-missing.md) — missing export summaries
