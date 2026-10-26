// kui kernel/Select.tsx (used at shell/SettingsPage.tsx:79, MessageFilterBar.tsx:584): the child reads its props through a `merge` view and
// invokes `onChange` from a nested helper, so the callbacks a caller passes are not proven to run in a known place.
import { createSignal, merge } from "solid-js";

type SelectProps = {
  readonly label: string;
  readonly size?: "sm" | "md";
  readonly onChange?: (value: string) => void;
};

function Select(props: SelectProps) {
  const p = merge({ size: "md" } as const, props);
  function commit(value: string): void {
    props.onChange?.(value);
  }
  return (
    <button class={p.size} onClick={() => commit("light")}>
      {props.label}
    </button>
  );
}

type Theme = { select: (value: string) => void };

export function SettingsPage(props: { theme: Theme }) {
  const [last, setLast] = createSignal("");
  return (
    <>
      <Select label="Theme" onChange={(value) => props.theme.select(value)} />
      <Select label="Last" onChange={(value) => setLast(value)} />
      <p>{last()}</p>
    </>
  );
}
