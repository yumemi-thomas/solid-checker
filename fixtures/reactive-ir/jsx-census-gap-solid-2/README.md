# JSX census gaps — Solid 2

This fixture pins the rule that absence of a compiler execution fact is not
proof of untracked execution or deletion.

- A template-root void child and a static `<noscript>` child have no usable
  execution site and produce uncertifiable SC1001 findings.
- Under Ryan's authoritative `next` transform semantics, the child beside a
  dynamic `textContent` attribute is also uncensused. SC8003 independently
  reports the visible children/`textContent` authoring conflict.
- An ordinary tracked child remains silent.
- A read in the component body, outside JSX, remains a proven SC1001 violation.

The fixture intentionally follows `dom-expressions#next` semantics instead of
forcing Babel output parity; the checker consumes whatever execution facts that
compiler truthfully provides and fails closed over missing ones.
