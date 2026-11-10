# latest and isPending under the observer guard

ADR 0941. `latest(fn)` and `isPending(fn)` run `fn()` under the caller's owner and observer (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js:1654`, `:1699`). `resolve` and `until` inside `latest` throw the guard's error (SC2004, SC2005; Chrome, five sites, and since ADR 0944 the helper's call, where the leaf walk reaches the same throw, is a sixth finding); `isPending` catches it, and `untrack` and a handler have no observer (quiet).
