# JSX census gaps — Solid 2

This fixture pins the rule that absence of a compiler execution fact is not
proof of untracked execution or deletion.

- A template-root void child is positively reported as discarded and remains
  silent. A static `<noscript>` child still has no usable execution site and
  produces an uncertifiable SC1001 finding.
- Under the current Solid compiler, the child beside a dynamic `textContent`
  attribute is positively tracked. SC8003 independently reports the visible
  children/`textContent` authoring conflict.
- An ordinary tracked child remains silent.
- A read in the component body, outside JSX, remains a proven SC1001 violation.

The fixture follows the pinned `solidjs/solid` compiler semantics instead of
forcing compatibility with the former DOM Expressions fork; the checker
consumes the compiler's positive execution facts and fails closed over genuine
missing ones.
