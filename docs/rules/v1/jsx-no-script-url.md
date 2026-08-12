# v1/jsx-no-script-url

`SC8004` · **error** · violation

A statically known JSX attribute value uses the `javascript:` URL scheme.

## What it does

Checks literal attribute values, expressions that fold to static strings, and
literal-string types recovered through TypeScript. Matching follows browser URL
normalization: it ignores leading controls, embedded tabs and newlines, letter
case, and common HTML character references such as `&colon;`. Dynamic strings
whose value cannot be proven are not guessed at.

## Why is this bad?

Script URLs mix navigation with code execution and are a common injection sink.
Solid does not treat them as an event-handler mechanism, so they are also often a
React-era or HTML-era idiom that only appears to work.

## Examples

Incorrect:

```tsx
<a href="java&#x09;script:removeAccount()">Remove</a>
```

Correct:

```tsx
<button type="button" onClick={removeAccount}>Remove</button>
```

## How to fix

Use a normal URL for navigation. If the intent is to run code, use the matching
Solid event prop such as `onClick` or `onSubmit`, and keep navigation and side
effects explicit. No automatic rewrite is offered because the correct event and
element semantics depend on the surrounding UI.

## Related

- [event-handlers](event-handlers.md) — Solid event-handler spellings
