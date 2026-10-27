// kui feature-messages MessageFilterBar.tsx:455,461,534-604 (FieldPredicateControl): a child calls its callback prop from one helper that a
// merge-nested-helper Select, a timer and a key handler all call, so the callers disagree and the callback's reads are not placed.
import { merge } from "solid-js";

type SelectProps = { readonly size?: "sm" | "md"; readonly onChange?: (value: string) => void };

function Select(props: SelectProps) {
  const p = merge({ size: "md" } as const, props);
  function commit(value: string): void {
    props.onChange?.(value);
  }
  return <button class={p.size} onClick={() => commit("contains")} />;
}

function FieldControl(props: { readonly onChange: (text: string | undefined) => void }) {
  let debounce: ReturnType<typeof setTimeout> | undefined;
  function commit(text: string): void {
    props.onChange(text === "" ? undefined : text);
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

export function FilterBar(props: { readonly predicates: Record<string, string>; readonly onChange: (next: Record<string, string>) => void }) {
  return <FieldControl onChange={(key) => props.onChange({ ...props.predicates, key: key ?? "" })} />;
}
