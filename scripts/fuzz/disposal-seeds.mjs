// Known shapes of the disposal profile, pinned: each is a plan for `generateProgram(seed, { profile: "disposal", disposalPlan })`
// and the verdict the 2.0.0-rc.17 runtime gives (checked by `node scripts/fuzz/disposal-seeds.mjs`, which runs them through the fast
// path and prints what the runtime and the checker say; disposal.test.mjs pins the printed shapes).
//
// `throws` is what the runtime does: true when it reports REACTIVE_WRITE_IN_OWNED_SCOPE (or the action code) at the write.
export const SEEDS = [
  { name: "P1 captured owner in a disposed Show", throws: false,
    plan: { via: "signal", where: "show", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-captured"], usesTrigger: false } },
  { name: "bare write in a disposed Show", throws: false,
    plan: { via: "signal", where: "show", cleanupVia: "onCleanup", handleMode: "discard", wrappers: [], usesTrigger: false } },
  { name: "createOwner() in a disposed Show", throws: false,
    plan: { via: "signal", where: "show", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-create"], usesTrigger: false } },
  { name: "write after an await, owner restored", throws: true,
    plan: { via: "signal", where: "show", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-captured", "async-before"], usesTrigger: false } },
  { name: "handle called later from a button", throws: true,
    plan: { via: "signal", where: "rootd", cleanupVia: "onCleanup", handleMode: "call-later", wrappers: ["owner-captured"], usesTrigger: false } },
  { name: "handle called at once in the host", throws: true,
    plan: { via: "signal", where: "show", cleanupVia: "onCleanup", handleMode: "call-now", wrappers: [], usesTrigger: false } },
  { name: "effect returned cleanup, rerun by a tracked read", throws: true,
    plan: { via: "signal", where: "body", cleanupVia: "returned", handleMode: "none", wrappers: ["owner-captured"], usesTrigger: true } },
  { name: "effect returned cleanup at teardown (Show off)", throws: false,
    plan: { via: "signal", where: "show", cleanupVia: "returned", handleMode: "none", wrappers: ["owner-captured"], usesTrigger: false } },
  { name: "action call in a disposed Show", throws: true,
    plan: { via: "action", where: "show", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-captured"], usesTrigger: false } },
  { name: "store setter in a disposed createRoot", throws: false,
    plan: { via: "store", where: "rootd", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-captured"], usesTrigger: false } },
  { name: "refresh in a rerun memo", throws: false,
    plan: { via: "refresh", where: "memo", cleanupVia: "onCleanup", handleMode: "discard", wrappers: ["owner-captured"], usesTrigger: true } },
  { name: "control: write in a component body", throws: true, plan: { via: "signal", control: "control-body", wrappers: [] } },
  { name: "control: write in an event handler", throws: false, plan: { via: "signal", control: "control-handler", wrappers: [] } },
];
