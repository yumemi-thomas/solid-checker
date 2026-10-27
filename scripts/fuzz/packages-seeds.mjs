// Known shapes of the packages profile, pinned: each is a plan for `generateProgram(seed, { profile: "packages", packagePlan })` and the
// verdict the 2.0.0-rc.17 runtime gives (checked by `node scripts/fuzz/packages-check.mjs`, which runs them through the fast path and
// prints what the runtime and the checker say; packages.test.mjs pins the printed shapes).
//
// `throws` is what the runtime does at the write: true when it reports REACTIVE_WRITE_IN_OWNED_SCOPE. `ran: false` says the write never runs. `plain` leaves out decoys and
// the outer wrapper, so the program is the shape and nothing else.
const pkg = (behaviour, exportForm = "named", extra = {}) => ({ k: "pkg", pkg: { behaviour, exportForm, ...extra } });
const plan = (host, layers, extra = {}) => ({ host, layers, kind: "write", disp: "run", plain: true, ...extra });

export const SEEDS = [
  // The three shapes of the brief.
  { name: "inline call with a write in a memo", throws: true, plan: plan("memo", [pkg("inline")]) },
  { name: "setTimeout call with a write in a memo", throws: false, plan: plan("memo", [pkg("timer")]) },
  { name: "stored, then called from a handler", throws: false, plan: plan("body", [pkg("register")], { fire: "handler" }) },
  // The same behaviours elsewhere.
  { name: "inline call with a write in a component body", throws: true, plan: plan("body", [pkg("inline")]) },
  { name: "inline call with a write in an event handler", throws: false, plan: plan("handler", [pkg("inline")]) },
  { name: "repeated call with a write in a component body", throws: true, plan: plan("body", [pkg("repeat", "arrow", { n: 3 })]) },
  { name: "queueMicrotask call with a write in a component body", throws: false, plan: plan("body", [pkg("micro", "default")]) },
  { name: "Promise.then call with a write in a component body", throws: false, plan: plan("body", [pkg("then", "reexport")]) },
  { name: "stored, then called in the component body", throws: true, plan: plan("body", [pkg("register", "class")], { fire: "body" }) },
  { name: "stored with its owner, called from a handler (the owner is restored, so the write is owned)", throws: true, plan: plan("body", [pkg("register", "object", { owned: true, store: "array" })], { fire: "handler" }) },
  { name: "never called", throws: false, ran: false, plan: plan("body", [pkg("never", "namespace")]) },
  { name: "returned, then called from a handler", throws: false, plan: plan("body", [pkg("returnCb", "star")], { disp: "handler" }) },
  { name: "returned wrapper, called in the component body", throws: true, plan: plan("body", [pkg("returnWrap", "static")], { disp: "call" }) },
  { name: "inline call under untrack in a memo", throws: true, plan: plan("memo", [pkg("untrack")]) },
  { name: "createEffect apply in a component body", throws: false, plan: plan("body", [pkg("effectApply", "reexportRenamed")]) },
  { name: "createRoot call in a memo", throws: true, plan: plan("memo", [pkg("root")]) },
];
