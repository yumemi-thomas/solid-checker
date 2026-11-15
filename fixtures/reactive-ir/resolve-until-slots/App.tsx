// ADR 0985: every `resolve` and `until` call ends in a row, or in a silence that names why no observer is active where it runs, or in an
// uncertifiable row. Each component below pins one cell; the twins are `resolve` (SC2004) and `until` (SC2005) in the same place.
// Chrome rc.21 (probed): `isPending(() => resolve(..))` in a memo, `untrack`, `runWithOwner(getOwner(), ..)` and `createRoot` are quiet; `latest(() => resolve(..))` throws.
import * as S from "solid-js";
import { action, createMemo, createRoot, createSignal, getOwner, isPending, latest, resolve, runWithOwner, until, untrack } from "solid-js";

const [n] = createSignal(1);
const wait = () => n();
const resolveAlias = resolve;
const untilAlias = until;

// ---------------------------------------------------------------------------------------------------------------- resolve

// V: `latest` keeps the observer, so the guard throws in the memo.
export function LatestInMemoR() {
  const m = createMemo(() => { void latest(() => resolve(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// V: a const alias of the import is the primitive.
export function AliasInMemoR() {
  const m = createMemo(() => { void resolveAlias(wait); return n(); });
  return <div>{String(m())}</div>;
}

// V: the namespace member is the primitive.
export function NamespaceInMemoR() {
  const m = createMemo(() => { void S.resolve(wait); return n(); });
  return <div>{String(m())}</div>;
}

// U: an async callback of `isPending` keeps ADR 0959's obligation (the rejection is not caught).
export function PendingAsyncInMemoR() {
  const m = createMemo(() => { void isPending(async () => { await resolve(wait); return false; }); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-swallows-throw: `isPending` takes what its synchronous callback throws.
export function PendingSyncInMemoR() {
  const m = createMemo(() => { void isPending(() => { void resolve(() => n()); return false; }); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function UntrackInMemoR() {
  const m = createMemo(() => { untrack(() => resolve(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function RunWithOwnerInMemoR() {
  const m = createMemo(() => { runWithOwner(getOwner(), () => resolve(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function RootInMemoR() {
  const m = createMemo(() => { createRoot(() => resolve(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, role-deferred-callback: a timer runs it on a fresh stack.
export function TimerInBodyR() {
  setTimeout(() => { void resolve(wait); });
  return <div />;
}

// silent, past-suspension: past an await every path reaches there is no observer.
export function AfterAwaitInMemoR() {
  createMemo(async () => { await Promise.resolve(); void resolve(wait); return 1; });
  return <div />;
}

// no site: a local function named like the primitive is not the primitive.
export function ShadowedInMemoR() {
  const resolve = (f: () => number) => f();
  const m = createMemo(() => { void resolve(wait); return n(); });
  return <div>{String(m())}</div>;
}

// silent, step-resumed: the step runs from the continuation of a promise it yielded.
export const resumedR = action(function* () {
  yield Promise.resolve();
  void resolve(wait);
});

// silent, step-callers-untracked: the only invocation is a handler, which no observer runs.
const steppedR = action(function* () {
  void resolve(wait);
});
export function HandlerOnlyR() {
  return <button onClick={() => steppedR()}>go</button>;
}

// silent, step-callers-untracked: the action is handed to a handler as a value, so no call stands anywhere (the step itself is the premise).
const valuedR = action(function* () {
  void resolve(wait);
});
export function HandlerValueR() {
  return <button onClick={valuedR}>go</button>;
}

// ---------------------------------------------------------------------------------------------------------------- until

// V: `latest` keeps the observer, so the guard throws in the memo.
export function LatestInMemoU() {
  const m = createMemo(() => { void latest(() => until(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// V: a const alias of the import is the primitive.
export function AliasInMemoU() {
  const m = createMemo(() => { void untilAlias(wait); return n(); });
  return <div>{String(m())}</div>;
}

// V: the namespace member is the primitive.
export function NamespaceInMemoU() {
  const m = createMemo(() => { void S.until(wait); return n(); });
  return <div>{String(m())}</div>;
}

// U: an async callback of `isPending` keeps ADR 0959's obligation (the rejection is not caught).
export function PendingAsyncInMemoU() {
  const m = createMemo(() => { void isPending(async () => { await until(wait); return false; }); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-swallows-throw: `isPending` takes what its synchronous callback throws.
export function PendingSyncInMemoU() {
  const m = createMemo(() => { void isPending(() => { void until(() => n()); return false; }); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function UntrackInMemoU() {
  const m = createMemo(() => { untrack(() => until(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function RunWithOwnerInMemoU() {
  const m = createMemo(() => { runWithOwner(getOwner(), () => until(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, slot-clears-observer.
export function RootInMemoU() {
  const m = createMemo(() => { createRoot(() => until(wait)); return n(); });
  return <div>{String(m())}</div>;
}

// silent, role-deferred-callback: a timer runs it on a fresh stack.
export function TimerInBodyU() {
  setTimeout(() => { void until(wait); });
  return <div />;
}

// silent, past-suspension: past an await every path reaches there is no observer.
export function AfterAwaitInMemoU() {
  createMemo(async () => { await Promise.resolve(); void until(wait); return 1; });
  return <div />;
}

// no site: a local function named like the primitive is not the primitive.
export function ShadowedInMemoU() {
  const until = (f: () => number) => f();
  const m = createMemo(() => { void until(wait); return n(); });
  return <div>{String(m())}</div>;
}

// silent, step-resumed: the step runs from the continuation of a promise it yielded.
export const resumedU = action(function* () {
  yield Promise.resolve();
  void until(wait);
});

// silent, step-callers-untracked: the only invocation is a handler, which no observer runs.
const steppedU = action(function* () {
  void until(wait);
});
export function HandlerOnlyU() {
  return <button onClick={() => steppedU()}>go</button>;
}

// silent, step-callers-untracked: the action is handed to a handler as a value, so no call stands anywhere (the step itself is the premise).
const valuedU = action(function* () {
  void until(wait);
});
export function HandlerValueU() {
  return <button onClick={valuedU}>go</button>;
}
