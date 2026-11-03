# past-await-owner-observer

**Claim (ADR 0612).** Past an await an async compute has neither its observer nor its owner (rc.17 reads both as ambient
state, and both are restored before a promise job runs). So `resolve` and `until` there are legal (`Observer`; a Solid
guard on `getObserver()`), directly or through a function run in place; past a possible await they are uncertifiable, and
before the await they stay violations. A compute's region owns its synchronous run only: `createEffect` and `onCleanup`
past the await are proven `missing-owner` (`Owner`; the region used to answer for them, a certified false clean), and
past a `try` head that may throw they are uncertifiable. The object of a written member (`s.a` in `s.a.v = 1`) is read
past the await (`reactive-read-after-await`, as ADR 0498 has it in a strict-read window) unless a read before the await
tracks it (`Link`), and a member read in a function run in place past the await is read there (`LayeredRead`). All measured on rc.17's fast runtime. The stub is copied from `after-await-helper-callback` with
`resolve` and `until` from the published `@solidjs/signals@2.0.0-rc.17` typings.
