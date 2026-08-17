# v1/event-handlers

`SC8001` · **warning** · violation

Validates Solid JSX event-handler spelling and values from Oxc JSX facts.

For a value that is neither a directly written string nor an obviously static
string local, the static-value branch follows the pinned 1.x compiler: only a
`StringLiteral` or `NumericLiteral` expression is frozen into the template.
Thus `onClick={-1}` and `onClick={NaN}` are not treated as static merely because
TypeScript renders both as `number`; radix and separator numeric literal syntax
is still a numeric literal.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/event-handlers": { "ignoreCase": false, "warnOnSpread": false }
  }
}
```

- `ignoreCase` (default `false`) — accept handler names as written; the
  canonical-spelling and ambiguous-name advice is off.
- `warnOnSpread` (default `false`) — report handler-named properties carried
  into a DOM element through a JSX spread, which Solid does not attach as
  listeners.
