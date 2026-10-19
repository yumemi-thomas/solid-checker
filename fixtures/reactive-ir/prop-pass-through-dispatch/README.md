# prop-pass-through-dispatch

**Claim (ADR 0214).** In a closed program, a parameter-member obligation whose argument is `props.name` is discharged when the prop holds a built-in value on every render: `props` is the only parameter and unwritten, the component is rendered only through resolved JSX tags that spread nothing and pass `name` as a string, boolean, `value_origin`-proven value or a parent's parameter/prop holding one. `List` clean; `Spread`, `Mixed`, `Routed` (exported in an array) SC9012. Scaffolding from `parameter-pass-through-dispatch`.
