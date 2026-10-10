# fp-exec-await-argument-write

**Claim.** In an async function, a write whose own argument awaits
(`setRows(await fetchRows())`) runs after the await, when the function has no
owner, so it is not a write in the owned body that called the function. A
call's arguments are evaluated before the call runs
(`execution_role::follows_await_in_async_function`). Positive control: a write
before the first await runs during the call and stays a proven
`reactive-write-in-owned-scope`.

Found on the held-out sweep (`rice-pos-solidjs2`, ten sites, and aiui
`MicPicker`). The stub is copied from `fp-exec-handler-writes`. `App.tsx`
passes `tsc --noEmit` against the stub and the real rc.9 install.
