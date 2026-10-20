# rc13-flush-in-effect-server

On a server build of the Solid packages an effect's apply never runs and `flush` is empty (`solid-js` `dist/server.js`: `createEffect` is `serverEffect(compute, undefined, options)`, `flush() {}`), so the client-runtime claim of SC2008 does not apply: the same code that `rc13-flush-in-effect-callback` reports is silent under `"rendering": "string-ssr"` (`.solid-checker/runtime.json`), and so for a Node-family target or `streaming-ssr`. No selection is the client default.
