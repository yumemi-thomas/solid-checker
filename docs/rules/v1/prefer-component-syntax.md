# v1/prefer-component-syntax

`SC8018` · **warning** · violation

Requires a lowercase local function that directly returns JSX to be expressed
as a component when it is rendered from JSX. The check follows TypeScript
symbol identity, so shadowed same-spelled helpers and functions returning
ordinary values remain valid.

```tsx
const renderHeader = () => <header>Header</header>;
const Panel = () => <article>{renderHeader()}</article>; // Incorrect

const Header = () => <header>Header</header>;
const FixedPanel = () => <article><Header /></article>; // Correct
```

Uppercase JSX syntax makes the component boundary visible and preserves Solid
1.x's component conventions for props, ownership, and one-time setup. Rename
the function and replace imperative calls with an element; if it is truly a
value helper, make it return data instead of JSX.

The version-independent rationale is also documented under
[prefer-component-syntax](../prefer-component-syntax.md).
