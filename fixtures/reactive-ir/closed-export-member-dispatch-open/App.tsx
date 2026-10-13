import { Card, describe, escapes } from "./helpers";

export function Shown(props: { item: { label(): string } }) {
  // The argument is one object literal: its `label` method is what runs.
  const named = describe({ label() { return "fixed"; } });
  const other = escapes({ label() { return "fixed"; } });
  // A caller-supplied value: this call site keeps its own obligation.
  const passed = describe(props.item);
  return (
    <p>
      {named}
      {other}
      {passed}
      <Card format={{ label: () => "fixed" }} />
    </p>
  );
}
