// ADR 0366 and ADR 0361 (amended), observed in Chrome on the rc.17 dev build.
// Each handler records, on `window.ownerless`, the owner it ran under while one
// of the case's own dispatches was on the stack.
import { getOwner, onCleanup } from "solid-js";

declare global {
  interface Window {
    ownerless?: Record<string, string>;
  }
}
const seen: Record<string, string> = (window.ownerless = {});
let dispatching = "";
const owner = () => (getOwner() === null ? "unowned" : "owned");

// A dispatch after an `await` that dominates it, in an async function the
// component body starts (owned before the await). The `release` click resolves
// the gate; the continuation's `target.click()` runs the click handler with no
// owner, so its `onCleanup` is a proven violation (no site fires `click`).
export function AfterAwait() {
  let target!: HTMLButtonElement;
  let release!: () => void;
  const gate = new Promise<void>((done) => (release = done));
  const run = async () => {
    seen["before-await"] = owner();
    await gate;
    dispatching = "after-await";
    target.click();
    dispatching = "";
  };
  void run();
  return (
    <div>
      <button data-probe="release" onClick={() => release()}>release</button>
      <button
        ref={(el) => (target = el)}
        onClick={() => {
          if (dispatching === "after-await") seen["after-await"] = owner();
          onCleanup(() => undefined);
        }}
      >
        target
      </button>
    </div>
  );
}

function Scroller(props: { onScrolled: () => void; ref: (el: HTMLDivElement) => void }) {
  return (
    <div ref={props.ref} onScroll={props.onScrolled}>
      scroll
    </div>
  );
}

// A callback forwarded through a component prop to `onScroll`, beside a
// `focus()` site (the focus move fires no `scroll`): the forwarded handler is
// reached by a `scroll` (here one a microtask dispatches, on an empty stack)
// and never by the focus dispatch, so its `onCleanup` is a proven violation.
export function Forwarded() {
  let input!: HTMLInputElement;
  let scroller!: HTMLDivElement;
  return (
    <div>
      <Scroller
        ref={(el) => (scroller = el)}
        onScrolled={() => {
          seen.forwarded = `${seen.forwarded ?? ""}${dispatching}:${owner()};`;
          onCleanup(() => undefined);
        }}
      />
      <input ref={(el) => (input = el)} />
      <button
        data-probe="focus"
        onClick={() => {
          dispatching = "focus";
          input.focus();
          dispatching = "";
        }}
      >
        focus
      </button>
      <button
        data-probe="scroll-microtask"
        onClick={() =>
          queueMicrotask(() => {
            dispatching = "microtask";
            scroller.dispatchEvent(new Event("scroll"));
            dispatching = "";
          })
        }
      >
        scroll
      </button>
    </div>
  );
}
