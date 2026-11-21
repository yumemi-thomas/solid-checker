# for-await-body

**Claim (ADR 1038).** The body of a `for await` runs only after the loop awaited the iterator's first `next()`, so it is past a suspension with no owner: a write there is clean (`WriteInBody`, uncertifiable on main). Not this ADR's region, unchanged: the iterable (`WriteInIterable`, evaluated before the suspension: a violation), a `finally` around the loop (`WriteInFinally`: the iterable may throw synchronously), code after the loop (`WriteAfterLoop`). `CleanupInBody` stayed uncertifiable here (an exported component's callers are not enumerated); ADR 1165 makes it a violation: the body over `[1]` runs, every run past the loop's await, with no owner whoever calls the component.

Chrome: `fixtures/runtime-oracle/for-await-body`. Stubs: `dead-site-unused-class`'s. `tsc --noEmit` clean.
