import { onCleanup } from "solid-js";

// `click` is delegated and bubbles, so a `click()` joins no handler. (An
// argumentless `click()` is resolved to its declaration only where a demand
// plan asks, here a call through a parameter; see docs/open-work.md.)
function press(target: HTMLButtonElement) {
  target.click();
}

export function Click() {
  const el = (<button onClick={() => { onCleanup(() => undefined); }} />) as HTMLButtonElement;
  press(el);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
