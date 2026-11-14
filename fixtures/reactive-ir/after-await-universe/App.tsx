import {
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  createStore,
  getOwner,
  runWithOwner,
  runWithOwner as withOwner,
  untrack,
} from "solid-js";
import type { Owner } from "solid-js";
import * as solid from "solid-js";
import * as helpers from "./helpers";

const [count, setCount] = createSignal(0);
const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): every read and every call that may run a project function, written after an async function's own suspension, ends in a row of SC1002, an
// SC9012, the compiler's discard or a typed reason, or it gets SC1002's uncertifiable row (blocker `read-after-await-unaccounted`): never a violation, never a silence. The
// producers walk only from a tracked async computation, so each site below was silent before the universe: nothing walks it and nothing said why. There is no callee reason: a
// call keeps its row whatever its callee does (the review of #233, B1).
//
// This program holds a tracked async computation with something after a suspension (`Tracked`), and no site below is that computation's own.
export function Tracked() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(
    () => m(),
    () => {},
  );
  return <div />;
}

// A read after an await in an async function no computation runs: an event handler (a role silence is left out on purpose, so the site keeps its row) and a helper.
export function Handler() {
  return (
    <button
      onClick={async () => {
        await pause();
        count();
      }}
    >
      go
    </button>
  );
}
export async function helper() {
  await pause();
  return count();
}

// A call of a project function after an await: it calls the standard library, reads a source, or calls itself. (There is no callee reason: the row stays even for a callee that
// reads nothing, see `after-await-universe-none`.)
function stringify(n: number) {
  return JSON.stringify({ n });
}
function readsCount() {
  return count();
}
function ping(n: number): number {
  return n > 0 ? pong(n - 1) : 0;
}
function pong(n: number): number {
  return ping(n);
}
export async function Calls() {
  await pause();
  stringify(1);
  readsCount();
  ping(3);
}

// The same through a namespace import: the callee is another module's function, resolved exactly, whose closure calls the standard library.
export async function Namespaced() {
  await pause();
  helpers.format(1);
}

// A call of a function value after an await (a parameter), and a call of a function that is not exactly resolved (a reassigned `let`).
export async function Invokes(callback: () => unknown) {
  await pause();
  callback();
}
let reassigned = () => 1;
reassigned = () => count();
export async function Reassigned() {
  await pause();
  reassigned();
}

// A site in a function a reviewed test runner schedules is a site like any other (owner ruling 9b): this one has no runner here, so it is a plain async function.
export async function Loop(items: number[]) {
  for await (const item of items) {
    count();
    void item;
  }
}

// A call of a project method after an await (owner ruling, slice 9B decision on members). A method of a `const` object literal nothing writes a member of is exact (ADR 0279):
// the call is a site and the callee summary applies; here the closure calls the standard library or reads a source, so the row stays.
const codec = {
  encode(n: number) {
    return JSON.stringify({ n });
  },
};
const meter = {
  read() {
    return count();
  },
};
export async function ByExactMember() {
  await pause();
  codec.encode(1);
  meter.read();
}

// A method an override or a replacement may stand in for is no exact callee: `this.helper()` in a class's own async method, and a receiver that is a parameter typed as the
// class. The call is a site whose row nothing ends, and no summary is asked of it.
class Loader {
  helper() {
    return 1;
  }
  async load() {
    await pause();
    return this.helper();
  }
}
export async function ByParameterReceiver(loader: Loader) {
  await pause();
  loader.helper();
}


// Review of #233, B1: a call keeps its row whatever its callee's closure reads. The closure of each `peek…` below lists no read of the universe, and Chrome rc.21 reports
// UNTRACKED_READ_AFTER_AWAIT under the call (the oracle case `after-await-closure`): a store read through an alias, a stored generator stepped by a spread, a Proxy trap and a
// getter installed through a handler or a descriptor declared apart (the inline spellings have a row of their own at the read, ADR 0963's member route).
const [store] = createStore({ x: 1, n: { x: 1 } });
const alias = store.n;
function peekAlias() {
  return alias.x;
}
function* numbers() {
  yield count();
}
const stored = numbers();
function peekStored() {
  return [...stored];
}
function reader() {
  return count();
}
const traps = { get: reader };
const proxy: any = new Proxy({}, traps);
function peekProxy() {
  return proxy.x;
}
const descriptor = { get: reader };
const installed: any = {};
Object.defineProperty(installed, "value", descriptor);
function peekDescriptor() {
  return installed.value;
}
export function ByClosure() {
  const data = createMemo(async () => {
    await pause();
    peekAlias();
    peekStored();
    peekProxy();
    peekDescriptor();
    return 1;
  });
  return <div>{String(data())}</div>;
}

// Review of #233, B2 and B3: calls the resolver names no project function, each of which may run one on this stack. They left the pass through a `continue`: a method of a
// module-scope `const` object literal the receiver census withholds (the function is handed by identifier to a wrapper, a start the producers' index does not follow), its
// computed and optional members and an arrow property; `f.call`, `f.apply`, `Reflect.apply` and a function literal called where it is written. Each is a site (`project`,
// `unresolved`, `apply`, `iife`) and keeps its row (the oracle case `after-await-calls`).
const registry = {
  run() {
    return count();
  },
  tick: () => count(),
};
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
async function work() {
  await pause();
  registry.run();
  registry["run"]();
  registry?.run();
  registry.tick();
  reader.call(undefined);
  reader.apply(undefined, []);
  Reflect.apply(reader, undefined, []);
  (() => count())();
  (function () {
    return count();
  })();
  await (async () => count())();
}
export function Starts() {
  track(work);
  return <div />;
}

// ADR 0976 (the follow-up to the reviews of #233: S2 and R1 to R4): a call into code outside the project is a site, and ends only where a typed fact shows it runs no project
// code. Each call below was silent: it left the pass through the arm that skipped the standard library, Solid's setters and actions, and constructs. Chrome rc.21 reports
// UNTRACKED_READ_AFTER_AWAIT under each (the oracle case `after-await-library`).
const target = new EventTarget();
target.addEventListener("x", () => {
  count();
});
const button = document.createElement("button");
button.addEventListener("click", () => {
  count();
});
const controller = new AbortController();
controller.signal.addEventListener("abort", () => {
  count();
});
const iterator: Iterator<number> = {
  next() {
    count();
    return { done: true, value: 1 };
  },
};
class Counts extends Map<string, number> {
  get(key: string) {
    count();
    return super.get(key);
  }
}
const counts: Map<string, number> = new Counts();
class Reads {
  value = count();
}
class Wraps {
  constructor(callback: () => number) {
    callback();
  }
}
export async function Library() {
  await pause();
  [1].forEach(() => {
    count();
  });
  Array.from([1], reader);
  new Promise<number>((done) => done(count()));
  setCount(() => count());
  setCount(reader);
  target.dispatchEvent(new Event("x"));
  button.click();
  controller.abort();
  stored.next(1);
  iterator.next(undefined);
  counts.get("x");
  new Reads();
  new Wraps(reader);
}

// R5: `runWithOwner`'s callback runs on this stack and keeps the after-await flag (the dialect's `callback_keeps_after_await_flag`; Chrome rc.21 reports a read there for a null
// owner or one taken after the suspension, and none for an owner captured before it, for `untrack` or for `createRoot`). The call is a site, ended only by an owner the function
// captured before its first suspension.
export async function Primitives(unproven: Owner | null) {
  const captured = getOwner();
  await pause();
  runWithOwner(captured, () => count());
  runWithOwner(null, () => count());
  runWithOwner(getOwner(), () => count());
  runWithOwner(unproven, () => count());
  untrack(() => count());
  createRoot(() => count());
}
// The same call spelled through a namespace import and an alias, and a project function of the same name in an inner scope (a call of a project function, whose row no
// primitive's reason ends).
export async function Spelled() {
  const owner = getOwner();
  await pause();
  solid.runWithOwner(null, () => count());
  withOwner(null, () => count());
  solid.runWithOwner(owner, () => count());
  withOwner(owner, () => count());
}
export async function Shadowed() {
  const runWithOwner = (owner: unknown, fn: () => number) => fn();
  const owner = getOwner();
  await pause();
  runWithOwner(owner, () => count());
}

// What stays without a row (`Outside`, each call an exactly resolved standard-library or Solid call whose receiver and arguments are proven inert, ADR 0976): a call with a
// literal or an exact data literal as its argument, a setter given a literal, a construct with none, an ECMAScript global's member, and a timer's callback (it runs on a task of
// its own: another stack). The process test `a_library_call_that_runs_no_project_code_is_no_site` pins them and the mutation test of `library-runs-nothing` files exactly them.
export async function Outside() {
  await pause();
  JSON.stringify({ n: 1 });
  Promise.resolve(1);
  setCount(1);
  new Date();
  Math.max(1, 2);
  Object.keys({ a: 1 });
  setTimeout(() => {
    count();
  }, 0);
}
