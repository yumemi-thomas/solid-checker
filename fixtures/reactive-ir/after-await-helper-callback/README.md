# after-await-helper-callback

**Claim (ADR 0485).** A callback a project helper calls in its own synchronous body on every completion (`layer_run`'s in-place answers at the call) runs on the continuation's stack: a read in it after an async computation's `await` is `reactive-read-after-await` (SC1002), through a same-file helper and one in another module. Not claimed: a helper that keeps its callback (it may run it later, anywhere). Silent: the read before the `await`. Stubs as in `local-alias-bindings`.
