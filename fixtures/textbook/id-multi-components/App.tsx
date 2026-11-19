import { For, createMemo, createSignal } from "solid-js";

function Item(props: { label: string; selected: boolean; onSelect: () => void }) {
  return (
    <li class={{ selected: props.selected }} onClick={() => props.onSelect()}>
      {props.label}
    </li>
  );
}

function Summary(props: { count: number; selected: string }) {
  return (
    <p>
      {props.count} items, selected {props.selected}
    </p>
  );
}

export function Picker() {
  const items = ["red", "green", "blue"];
  const [selected, setSelected] = createSignal("red");
  const count = createMemo(() => items.length);
  return (
    <div>
      <ul>
        <For each={items}>{(item) => <Item label={item} selected={selected() === item} onSelect={() => setSelected(item)} />}</For>
      </ul>
      <Summary count={count()} selected={selected()} />
    </div>
  );
}
