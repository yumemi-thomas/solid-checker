# `@solid-primitives/context@0.3.2` declaration-layout finding

Phase 21 classifies the remaining `@solid-primitives/context@0.3.2|solid1|only`
row as a confirmed upstream declaration defect. It is not a checker-resolvable
package-layout case.

The authenticated published archive has integrity
`sha512-6fvTtpK17PFHnUf/UOc1TzBjd+kLFjtA62aRFEm1kDP9ufTo7FYW2kUzQAWbfbRHi30yjBJtopbR8qd6nShwyg==`.
Its `dist/index.d.ts` contains this dependency import:

```ts
import type { ContextProviderComponent } from "../node_modules/solid-js/types/reactive/signal.js";
```

From `dist/index.d.ts`, that spelling resolves inside the package archive at
`node_modules/solid-js/types/reactive/signal.js`. The archive has no
`node_modules` member. In the real package-manager layout, `solid-js@1.9.14` is
a hoisted peer at the project `node_modules`, not a nested member of the
`@solid-primitives/context` archive.

The published-typing oracle used an otherwise ordinary hoisted layout with the
exact context archive and the corpus's published `solid-js@1.9.14` archive:

```text
packages/cli/node_modules/.bin/tsc --noEmit -p <oracle>/tsconfig.json
<oracle>/node_modules/@solid-primitives/context/dist/index.d.ts(2,47): error TS2307:
Cannot find module '../node_modules/solid-js/types/reactive/signal.js' or its corresponding type declarations.
```

The package's `@solid-primitives/source` condition separately targets the
unpublished `./src/index.ts`. Both applicable artifact cases therefore remain
refused. The checker must not manufacture a nested peer layout or report a
semantic finding for code whose published declarations already fail TypeScript.
