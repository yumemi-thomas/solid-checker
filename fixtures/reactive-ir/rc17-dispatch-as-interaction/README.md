# rc17-dispatch-as-interaction

rc.17 `dispatchAsInteraction(e, fn)` runs `fn()` inline (`web.js:484`, `web.dev.js:653`), so `fn` keeps the caller's owner, listener and strict-read window, like `flush(fn)` (`flush-read-role`). Named, namespace and by-name spellings.
