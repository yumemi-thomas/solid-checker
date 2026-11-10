# Primitive callbacks after an await

ADR 0944. After an `await` in an async computation a read is untracked. A callback of `latest`, `isPending`, `flush(fn)` or `runWithOwner(null, fn)` runs in place on that continuation, so a read in it is SC1002 (Chrome, `@solidjs/signals@2.0.0-rc.21`); `untrack` clears tracking on purpose and a root's callback has an owner: silent. `runWithOwner` with a value the analysis cannot see through is uncertifiable.
