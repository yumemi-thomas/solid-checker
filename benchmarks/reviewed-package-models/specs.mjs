// Reviewed against the exact published runtime files named below. These are
// assumed behavioral premises for warnings, never certification authority.
export const specs = [
  { package: "@solid-primitives/utils", version: "7.0.0-next.4", files: ["dist/index.js"], exports: {
    access: { browser: { invokesArgument: 0 }, node: { invokesArgument: 0 } },
    createMicrotask: { browser: { owner: "cleanup" } },
  } },
  { package: "@solid-primitives/raf", version: "4.0.0-next.2", files: ["dist/index.js"], exports: {
    createRAF: { browser: { owner: "cleanup", returns: "tuple0" }, node: { inert: true } },
    createMs: { browser: { owner: "cleanup", returns: "accessor" } },
  } },
  { package: "@solid-primitives/event-listener", version: "3.0.0-next.5", files: ["dist/eventListener.js", "dist/eventListenerMap.js"], exports: {
    createEventListener: { browser: { owner: "effect" }, node: { inert: true } },
    createEventListenerMap: { browser: { owner: "effect" }, node: { inert: true } },
    createEventSignal: { browser: { owner: "effect", returns: "accessor" }, node: { inert: true } },
  } },
  { package: "@solid-primitives/timer", version: "1.4.5-next.1", files: ["dist/index.js"], exports: {
    createTimer: { browser: { owner: "cleanup" }, node: { inert: true } },
    createTimeoutLoop: { browser: { owner: "cleanup" }, node: { inert: true } },
    createPolled: { browser: { owner: "cleanup", returns: "accessor" }, node: { inert: true } },
    createIntervalCounter: { browser: { owner: "cleanup", returns: "accessor" }, node: { inert: true } },
  } },
  // The final four packages exercise the same vocabulary and lowering code.
  { package: "@solid-primitives/media", version: "4.0.0-next.2", files: ["dist/index.js"], exports: {
    // makeEventListener uses tryOnCleanup. Outside a browser owner it leaves
    // the listener persistent; that is no unconditional owner requirement.
    createMediaQuery: { browser: { returns: "accessor" }, node: { inert: true } },
    createPrefersDark: { browser: { returns: "accessor" }, node: { inert: true } },
  } },
  { package: "@solid-primitives/date", version: "3.0.0-next.3", files: ["dist/primitives.js"], exports: {
    createDateNow: { browser: { owner: "effect", returns: "tuple0" }, node: { inert: true } },
    createDate: { browser: { returns: "tuple0" } },
  } },
  { package: "@solid-primitives/lifecycle", version: "1.0.0-next.2", files: ["dist/index.js"], exports: {
    // onSettled's out-of-band callback has no cleanup; source review supplies
    // no missing-owner premise for this export. Its ledger owner case stays open.
    createIsMounted: { browser: { returns: "accessor" }, node: { inert: true } },
  } },
  { package: "@solid-primitives/memo", version: "2.0.0-next.2", files: ["dist/index.js"], exports: {
    createLazyMemo: { browser: { returns: "accessor", trackedCallback: 0 }, node: { inert: true } },
    createReducer: { browser: { returns: "tuple0" } },
    createPureReaction: { browser: { owner: "cleanup" }, node: { inert: true } },
  } },
];
