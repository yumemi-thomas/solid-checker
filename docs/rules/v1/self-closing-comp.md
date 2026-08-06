# v1/self-closing-comp

Enforces self-closing syntax for elements without children.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/self-closing-comp": { "component": "all", "html": "all" }
  }
}
```

- `component` (`"all"` | `"none"`, default `"all"`) — whether childless
  components must self-close, or must not.
- `html` (`"all"` | `"void"` | `"none"`, default `"all"`) — the same for
  native elements; `"void"` requires self-closing only for void elements
  (`br`, `img`, ...) and explicit closing tags everywhere else.
