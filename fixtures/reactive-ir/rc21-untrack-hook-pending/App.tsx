import { createMemo, createRoot, createSignal, createStore, untrack } from "solid-js";
import { dynamic } from "@solidjs/web";

function useBare() {
  return createMemo(async () => 1);
}
const useArrow = () => createMemo(async () => 1);
function useBound() {
  const memo = createMemo(async () => 1);
  return memo;
}
function useSlot() {
  const memo = createMemo(async () => 1);
  return { memo };
}
function useSync() {
  return createMemo(() => 1);
}
function useEither(flag: boolean) {
  if (flag) return createMemo(async () => 1);
  return createMemo(() => 2);
}
function useStored() {
  const [store] = createStore(async () => ({ a: 1 }), { a: 0 });
  return store;
}

// A hook's memo is created under the body's owner: a plain read and an untrack
// of it both throw PENDING_ASYNC_UNTRACKED_READ, in every spelling.
export function PlainRead() {
  const bare = useBare();
  const arrow = useArrow();
  const one = bare();
  const two = arrow();
  return <p>{[one, two].length}</p>;
}

export function ThroughUntrack() {
  const bare = useBare();
  const bound = useBound();
  const slot = useSlot();
  const { memo } = useSlot();
  const one = untrack(() => bare());
  const two = untrack(() => bound());
  const three = untrack(() => slot.memo());
  const four = untrack(() => memo());
  const five = untrack(bare);
  return <p>{[one, two, three, four, five].length}</p>;
}

// The layers run on the untrack's stack.
export function Layers() {
  const memo = useBare();
  const one = untrack(() => (() => memo())());
  const read = () => memo();
  const two = untrack(() => read());
  return <p>{[one, two].length}</p>;
}

// A sync hook's read is the strict read, and under untrack silent.
export function SyncHook() {
  const memo = useSync();
  const one = memo();
  const two = untrack(() => memo());
  return <p>{[one, two].length}</p>;
}

// Two creations: nothing says which one a call returns.
export function Either() {
  const memo = useEither(true);
  const one = untrack(() => memo());
  const two = memo();
  return <p>{[one, two].length}</p>;
}

// A hook's derived store.
export function HookStore() {
  const store = useStored();
  const one = untrack(() => store.a);
  return <p>{one}</p>;
}

const shared = useBare();

// A module-scope instance, and one a root owns: `owner._parent !== context`, so
// the first suspends and the second stays open.
export function OtherOwner() {
  const one = untrack(() => shared());
  const held = createRoot(() => useBare());
  const two = untrack(() => held());
  return <p>{[one, two].length}</p>;
}

function read(get: () => number) {
  return untrack(() => get());
}

// A helper that reads its parameter under untrack: open at the hand-off.
export function Handed() {
  const memo = useBare();
  const sync = useSync();
  const one = read(memo);
  const two = read(sync);
  return <p>{[one, two].length}</p>;
}

// A labelled untrack opens a window of its own wherever it runs.
export function Labelled() {
  const memo = useBare();
  const one = untrack(() => memo(), "mine");
  const two = untrack(() => memo(), false);
  return <p>{[one, two].length}</p>;
}

// A hook of a plain signal holds nothing pending: no obligation.
const usePlain = () => {
  const [value] = createSignal(1);
  return { value };
};

export function PlainHook() {
  const { value } = usePlain();
  const one = untrack(() => value());
  return <p>{one}</p>;
}

// A sync memo over an async one is pending with it, and throws here: open.
export function OverAsync() {
  const inner = createMemo(async () => 1);
  const outer = createMemo(() => inner());
  const one = untrack(() => outer());
  return <p>{one}</p>;
}

// `dynamic` mounts this in no window: the read re-runs the body onto a fresh
// pending memo, the page never settles, and nothing is reported: open.
function Remounted() {
  const memo = useBare();
  const one = untrack(() => memo());
  return <p>{one}</p>;
}

export function DynamicMount() {
  const Mounted = dynamic(() => Remounted);
  return <Mounted />;
}
