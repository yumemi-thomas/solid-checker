import { createEffect, createMemo, untrack } from "solid-js";

// ADR 0323 (review round 9): a callback whose invocations run in *different*
// roles that all keep a read out of the render window (a tracked computation,
// an explicit untrack, an event, a deferred callback) is not reported: no
// invocation is a reporting role. One reporting invocation (the render body,
// an effect's apply) refuses the whole mix, as a single such role always did.
// Every component is exported, so a read the engine cannot place is an
// uncertifiable obligation.

// A lazy memo and an explicit untrack (the shape of a query hook's key).
function useKeyed(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  return { state: () => bound(), reload: () => untrack(options.key) };
}
export function TrackedAndUntrack(props: { id: string }) {
  const q = useKeyed({ key: () => `k:${props.id}` });
  return <div>{q.state()}</div>;
}

// A tracked memo and an event handler.
function useTrackedAndEvent(options: { key: () => string }) {
  const bound = createMemo(() => options.key());
  return { read: () => bound(), press: () => console.log(options.key()) };
}
export function TrackedAndEvent(props: { id: string }) {
  const q = useTrackedAndEvent({ key: () => `k:${props.id}` });
  return <div onClick={q.press}>{q.read()}</div>;
}

// The callback handed by name to a tracked slot (a single role: tracked).
function useTracked(f: () => string) {
  return createMemo(f);
}
export function SlotByName(props: { id: string }) {
  const m = useTracked(() => `k:${props.id}`);
  return <div>{m()}</div>;
}

// Refusals: one reporting invocation keeps the obligation.
function useKeyedAndBody(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  const first = options.key();
  return { state: () => bound(), first };
}
export function TrackedAndBody(props: { id: string }) {
  const q = useKeyedAndBody({ key: () => `k:${props.id}` });
  return <div>{q.first}{q.state()}</div>;
}

function useKeyedAndApply(options: { key: () => string }) {
  const bound = createMemo(() => options.key(), { lazy: true });
  createEffect(
    () => bound(),
    () => {
      console.log(options.key());
    },
  );
  return bound;
}
export function TrackedAndApply(props: { id: string }) {
  const q = useKeyedAndApply({ key: () => `k:${props.id}` });
  return <div>{q()}</div>;
}

// A reporting slot by name (an effect's apply): refused.
function useApply(f: () => void) {
  createEffect(() => 0, f);
}
export function SlotApply(props: { id: string }) {
  useApply(() => console.log(props.id));
  return <div />;
}

// Each component above is rendered with props no tag enumerates, so they are
// unknown, not the empty props of a component nothing renders (ADR 0373).
declare const trackedAndBodyProps: Parameters<typeof TrackedAndBody>[0];
export function TrackedAndBodyHost() {
  return <TrackedAndBody {...trackedAndBodyProps} />;
}
declare const trackedAndApplyProps: Parameters<typeof TrackedAndApply>[0];
export function TrackedAndApplyHost() {
  return <TrackedAndApply {...trackedAndApplyProps} />;
}
declare const slotApplyProps: Parameters<typeof SlotApply>[0];
export function SlotApplyHost() {
  return <SlotApply {...slotApplyProps} />;
}
