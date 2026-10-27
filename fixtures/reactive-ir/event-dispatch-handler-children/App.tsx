import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

function Frame(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

// The dispatching handler's element is written in a component's children.
export function Card() {
  return (
    <div>
      <Frame>
        <button onClick={() => input.click()} />
      </Frame>
      <button
        onClick={() => {
          setCount(count() + 1);
          createRenderEffect(() => count(), () => {});
        }}
      />
    </div>
  );
}
