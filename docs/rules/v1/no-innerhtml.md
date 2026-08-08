# v1/no-innerhtml

`SC8008` · **error** · violation

Validates `innerHTML` and React-style dangerous HTML properties from Oxc JSX
facts.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/no-innerhtml": { "allowStatic": true }
  }
}
```

- `allowStatic` (default `true`) — accept a value proven to be a static HTML
  string, whether written as a literal or proven through its TypeScript
  string-literal type. With `false`, every `innerHTML` value is reported as an
  injection surface.
