// app-game ui-components-examples cursor-emulation/index.tsx:257 (`seekTimeline`) and kui feature-messages MessageFilterBar.tsx:455: a child calls
// its callback prop from one helper that a merge-nested-helper Select, a timer and a key handler all call.
import { createSignal, merge } from "solid-js";

function Select(props: { readonly size?: "sm" | "md"; readonly onChange?: (value: string) => void }) {
  const p = merge({ size: "md" } as const, props);
  function commit(value: string): void {
    props.onChange?.(value);
  }
  return <button class={p.size} onClick={() => commit("contains")} />;
}

function FieldControl(props: { readonly onChange: (text: string) => void }) {
  let debounce: ReturnType<typeof setTimeout> | undefined;
  function commit(text: string): void {
    props.onChange(text);
  }
  return (
    <>
      <Select onChange={(next) => commit(next)} />
      <input
        onInput={(event) => {
          clearTimeout(debounce);
          debounce = setTimeout(() => commit(event.currentTarget.value), 300);
        }}
        onKeyDown={(event) => event.key === "Enter" && commit(event.currentTarget.value)}
      />
    </>
  );
}

export function FilterBar() {
  const [last, setLast] = createSignal("");
  return (
    <>
      <FieldControl onChange={(text) => setLast(text)} />
      <p>{last()}</p>
    </>
  );
}
