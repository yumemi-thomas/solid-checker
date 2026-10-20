# Built-in methods in a leaf scope

A call directly in an `onSettled` callback runs in that leaf scope, so the checker must know it creates no primitive and registers no cleanup (ADR 0192; resolved standard-library declaration): `getBoundingClientRect()` is clean; `dialog.focus()` dispatches focus events synchronously so it is `SC9012` (ADR 0329); `maybeDialog?.focus()` stays `SC9012` (no resolved call for optional calls); `renderer.setSize()` (declared object, no body) `SC9012`. Stub `leaf-owner`'s (published `onSettled`).
