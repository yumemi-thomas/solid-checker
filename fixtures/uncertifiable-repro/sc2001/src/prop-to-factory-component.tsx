// app-game pixijs-examples pages/GraphicsSvg.tsx:25-47 (`Graphics` is `createPixiComponent(...)` in solid-pixi/P.tsx:95): callbacks handed to a component
// that a project factory builds, so its declaration is a call rather than a function.
import { createSignal } from "solid-js";

type Handlers = { onmouseleave?: () => void; onmouseenter?: () => void };

function createPixiComponent(tag: string) {
  return (props: Handlers) => (
    <div data-tag={tag} onMouseLeave={() => props.onmouseleave?.()} onMouseEnter={() => props.onmouseenter?.()} />
  );
}

const Graphics = createPixiComponent("graphics");

export function GraphicsSvg() {
  const [toggle, setToggle] = createSignal(false);
  return (
    <>
      <Graphics onmouseleave={() => setToggle(false)} onmouseenter={() => setToggle(true)} />
      <p>{toggle() ? "on" : "off"}</p>
    </>
  );
}
