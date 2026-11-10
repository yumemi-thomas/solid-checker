# Primitive callbacks inside an action's step

ADR 0944. A callback of `untrack`, `latest`, `createRoot` or `runWithOwner` runs in place inside the step, so a `flush()` in it is FLUSH_IN_ACTION (SC2006; Chrome, `@solidjs/signals@2.0.0-rc.21`); `flush(fn)` throws before it runs `fn`, so only the outer call is a finding. `isPending` catches the plain throw: silent. A callback handed to a timer runs outside the step: silent.
