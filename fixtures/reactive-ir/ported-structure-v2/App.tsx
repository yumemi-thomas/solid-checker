import { createSignal } from "solid-js";

const staticItems = ["a", "b"];

function Widget(_props: { innerHTML?: string; children?: unknown }) {
  return <span />;
}

export function Ported(props: { ready: boolean; html: string }) {
  const [items] = createSignal(staticItems);
  const [ready] = createSignal(false);
  const rows = staticItems.map((item) => <li>{item}</li>);
  return (
    <main>
      <ul>{items().map((item) => <li>{item}</li>)}</ul>
      <section>{ready() && <strong>ready</strong>}</section>
      <div innerHTML={props.html}>fallback</div>
      <div onClick={() => {}} onclick={() => {}} />
      <button title={props.ready ? "yes" : "no"} />
      <Widget innerHTML={props.html}>component content is independent</Widget>
      <ol>{rows}</ol>
    </main>
  );
}
