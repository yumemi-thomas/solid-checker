# v1/event-handlers

`SC8001` · **warning** · violation

Validates Solid JSX event-handler spelling and values from Oxc JSX facts.

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
