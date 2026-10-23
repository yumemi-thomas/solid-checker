import { onCleanup } from "solid-js";

// ADR 0361 (amended): the narrow join fires no type program-wide; it delivers
// to the handlers of the one element it dispatches on. A callback forwarded to
// that element's `onSync` ends at an attribute of that element, so it may run
// on the dispatcher's stack and keeps its obligation, although no site fires
// `sync`. The callback forwarded to another element's `onSync` is not reached:
// its `onCleanup` is a proven violation.
function Target(props: { onSynced: () => void }) {
  const el = (<section onSync={props.onSynced} />) as HTMLElement;
  return <button onClick={() => el.dispatchEvent(new Event("sync"))}>sync</button>;
}

function Other(props: { onSynced: () => void }) {
  return <section onSync={props.onSynced} />;
}

export function App() {
  return (
    <div>
      <Target onSynced={() => { onCleanup(() => undefined); }} />
      <Other onSynced={() => { onCleanup(() => undefined); }} />
    </div>
  );
}
