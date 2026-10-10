# prop-pass-through-dispatch

**Claim (ADR 0214).** In a closed program (a private `package.json`, ADR 0193), a parameter-member obligation whose argument is `props.name` is discharged when the prop holds a built-in value on every render. That holds when:
- `props` is the component's only parameter and nothing writes it;
- the component is rendered only through JSX tags the graph resolves to it;
- every tag spreads nothing and passes `name` as a string, a boolean, a value `value_origin` proves, or a parent's parameter or prop that holds one.

| Component | Obligation at `evens(props.items)` | Why |
| --- | --- | --- |
| `List` | none | `items={[1, 2]}`, and `Middle`'s `items={props.items}` whose only tag passes `[3]` |
| `Spread` | `SC9012` | its tag spreads an object |
| `Mixed` | `SC9012` | one tag passes `App`'s prop, and nothing renders `App` |
| `Routed` | `SC9012` | it is exported in an array, a value escape |

The `SC1001` findings on `props.items` read in each body belong to the props rules and are not part of this claim. The scaffolding is copied from `parameter-pass-through-dispatch`.
