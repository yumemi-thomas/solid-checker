# Inferred host: server-scopes

Copy of `inferred-host-spa` where `Document`'s module-scope unowned `onCleanup` makes the server baseline comparison nonvacuous: it keeps reporting the existing ownership defect with no inferred evidence, and `Document` and the module directive file share no contract or summary with `App`. Keeps rc.9's exact `createEffect` declarations (callback return `(() => void) | void`).
