# v1/no-unknown-namespaces

Reports unknown JSX namespaces from Oxc JSX facts.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/no-unknown-namespaces": { "allowedNamespaces": [] }
  }
}
```

- `allowedNamespaces` (default `[]`) — extra namespace prefixes to accept on
  top of the dialect compiler's own vocabulary.
