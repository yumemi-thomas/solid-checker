# program-boundary-closed

`.solid-checker/runtime.json` asserts `programBoundary: "closed"` (the analyzed files are the whole program): user-supplied evidence that removes only the assumption of an unseen caller. `StaticOnly` (every visible caller static) and `setupRooted` (sole call inside `createRoot`) go from uncertifiable to silent; `Dynamic` and `setupBare` stay violations (it creates no findings); `PassedAsValue` stays uncertifiable (no blanket amnesty). Open-world twins: `engine/eslint-reactivity-*`, `eslint-plugin-corpus*`.
