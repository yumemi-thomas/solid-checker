# Runtime/declaration export disagreement

This fixture pins the public contract surface as the intersection of the
authenticated runtime and declaration export censuses.

- `runtimeOnly` exists in `index.js` but not `index.d.ts`; it is not a
  TypeScript-facing package export and must not become an identity refusal or a
  contract claim.
- `declarationOnly` exists in `index.d.ts` but not at runtime; it must not become
  a runtime contract claim.
- `runtimeFactory` exists on both axes and remains the sole proposed export.
  Its runtime implementation is callable while the published declaration says
  `number`, so the proposal follows executable behavior and keeps unproved call
  domains open for certification.

The declaration-axis census is additive resolution evidence and is replayed
from the published archive before certification. An absent census grants no
filtering authority. `namespace-export-surface` is the negative control: a
genuinely shared namespace that the exact binding resolver cannot yet bind must
still refuse rather than disappearing through this intersection.

This does not duplicate a TypeScript diagnostic. A consumer sees only the
published declaration file; TypeScript does not compare that declaration with
the package's JavaScript implementation.
