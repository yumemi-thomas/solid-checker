# classic-script-global-module

**Claim (ADR 0904), negative twin.** The same `function setTimeout` and `let Promise` in a module (it exports them) are its own: no global is replaced, so `App`'s `setTimeout` and `Promise.resolve().then` keep their standard timing.
