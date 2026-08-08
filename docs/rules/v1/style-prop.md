# v1/style-prop

`SC8017` · **warning** · violation

Validates Solid's JSX `style` property representation and CSS property names.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/style-prop": { "styleProps": ["style"], "allowString": false }
  }
}
```

- `styleProps` (default `["style"]`) — the prop names the rule inspects.
  Naming props replaces the default, so `["css"]` leaves `style` alone.
- `allowString` (default `false`) — accept string-valued style props instead
  of asking for an object.
