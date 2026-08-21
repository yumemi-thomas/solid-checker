# v1/jsx-no-undef

`SC8005` · **error** · violation

Reports an undefined custom directive used through Solid 1.x's `use:`
namespace.

TypeScript deliberately does not bind the local-name node of a namespaced JSX
attribute, so it cannot issue its ordinary missing-name diagnostic for
`use:tooltip`. The checker fills that narrow gap with Oxc's semantic scope
binder. A finding is emitted only when the binder explicitly proves that no
value-space declaration is visible at the directive use. Imports, hoisted
declarations, parameters, nested block scopes, and shadowing are all resolved
by symbol identity; a type-only declaration does not satisfy the runtime
directive reference.

```tsx
const App = () => <button use:tooltip />;
//                              ^ SC8005: tooltip has no value binding
```

Declare or import the directive in lexical scope:

```tsx
import tooltip from "./tooltip";

const App = () => <button use:tooltip />;
```

Despite the retained upstream name, this rule does not inspect JSX component
tags. Undefined component identifiers belong to TypeScript and must not be
reported twice. Solid 2.0 has no corresponding rule because its published JSX
types do not expose the 1.x `Directives` interface.
