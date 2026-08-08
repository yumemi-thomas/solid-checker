# v1/prefer-classlist

`SC8013` · **warning** · violation

Suggests `classList` for conditional class expressions.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/prefer-classlist": { "classnames": ["cn", "clsx", "classnames"] }
  }
}
```

- `classnames` (default `["cn", "clsx", "classnames"]`) — the helper names
  whose object-literal call in a `class` prop is rewritten to `classlist`.
  Naming helpers replaces the default list.
