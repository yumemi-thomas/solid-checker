import { createSignal, Show } from "solid-js";
import * as Solid from "solid-js";

// Clean: event handlers on an element written as a <Show> child are the
// element's, and run when the event fires, with no owner.
export function HandlerInShow() {
  const [hovered, setHovered] = createSignal<string | null>(null);
  const [start] = createSignal(true);
  return (
    <button type="button" title={hovered() ?? ""}>
      <Show when={start()}>
        <div id="in" onMouseEnter={() => setHovered("start")} onClick={() => setHovered("click")} />
      </Show>
      <div id="out" onMouseEnter={() => setHovered("x")} />
    </button>
  );
}

// Clean: the same through a namespace import.
export function HandlerInNamespaceShow() {
  const [hovered, setHovered] = Solid.createSignal<string | null>("a");
  return (
    <Solid.Show when={hovered()}>
      <div id="ns" onClick={() => setHovered(null)} />
    </Solid.Show>
  );
}

// Clean: a handler on an element the render callback returns.
export function HandlerInRenderCallback() {
  const [value, setValue] = createSignal<string | null>("a");
  return <Show when={value()}>{(current: () => string) => <div id="cb" onClick={() => setValue(current() + "!")} />}</Show>;
}

// Violation: a write that runs while the <Show> children render.
export function WriteInShowChildren() {
  const [hovered, setHovered] = createSignal<string | null>(null);
  const [start] = createSignal(true);
  return (
    <button type="button" title={hovered() ?? ""}>
      <Show when={start()}>
        {(() => {
          setHovered("render");
          return <div />;
        })()}
      </Show>
    </button>
  );
}

// Violation: a write in the render callback's own body.
export function WriteInRenderCallback() {
  const [value, setValue] = createSignal<string | null>("a");
  return (
    <Show when={value()}>
      {(_current: () => string) => {
        setValue("b");
        return <div />;
      }}
    </Show>
  );
}
