import { createMemo, createSignal, flush, latest, merge, onCleanup } from "solid-js";

declare function load(): Promise<number>;
declare const client: { subscribe(listener: () => void): () => void; snapshot(): number };

// ---- Negatives (the five held-out shapes): none runs in the component body.

// A primitive's inline callback inside a component's callback prop runs when
// the component calls the prop.
function Header(props: { onToggle: () => void }) {
  return <span>{props.onToggle.length}</span>;
}
export function FlushInCallbackProp() {
  const [dock, setDock] = createSignal("bottom");
  return <Header onToggle={() => flush(() => setDock(dock() === "bottom" ? "right" : "bottom"))} />;
}

// latest() inside a helper that only runs after an await, and in a default
// parameter that is evaluated only when the function is called.
export function LatestInLaterHelper() {
  const [draft] = createSignal("a");
  const isCurrent = (id: string) => id === latest(() => draft());
  const save = async (id: string = latest(() => draft())) => {
    await load();
    return isCurrent(id);
  };
  return <span>{String(save.length)}</span>;
}

// merge() wraps a function source in a memo of its own.
export function MergeFunctionSource() {
  const [label] = createSignal("provided");
  const props = merge({ label: "default" }, () => ({ label: label() }));
  return <span>{props.label}</span>;
}

// A write in a listener handed to an unknown subscriber inside a memo runs
// when the subscriber calls it, not in the memo's tracking pass.
export function WriteInSubscriberListener() {
  const [snapshot, setSnapshot] = createSignal(0);
  createMemo(() => {
    const off = client.subscribe(() => setSnapshot(client.snapshot()));
    onCleanup(off);
    return 0;
  });
  return <span>{snapshot()}</span>;
}

// ---- Positives: the same reads made while the body runs stay reported.

export function LatestInBody() {
  const [draft] = createSignal("a");
  const current = latest(() => draft());
  return <span>{current}</span>;
}

export function DefaultParameterCalledInBody() {
  const [draft] = createSignal("a");
  const read = (value: string = draft()) => value;
  const current = read();
  return <span>{current}</span>;
}
