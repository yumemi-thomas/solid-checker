// app-game paint ui-example/trigonometry.tsx:156-170 (used at :37, :52, :67; kui DataTable.stories.tsx:145 is the form without `omit`): a child
// snapshots a callback prop with `untrack(() => props.onChange)`, so the value leaves the walk through `props.onChange`.
import { createSignal, omit, untrack } from "solid-js";

type InputProps = { value: number; onChange: (value: number) => void };

function Input(props: InputProps) {
  const others = omit(props, "onChange");
  const onChange = untrack(() => props.onChange);
  return <input {...others} ref={(el) => el.addEventListener("change", () => onChange(1))} />;
}

function Plain(props: InputProps) {
  const onChange = untrack(() => props.onChange);
  return <input value={props.value} ref={(el) => el.addEventListener("change", () => onChange(1))} />;
}

export function Page() {
  const [angle, setAngle] = createSignal(30);
  const [other, setOther] = createSignal(0);
  return (
    <>
      <Input value={angle()} onChange={(value) => setAngle(value)} />
      <Plain value={other()} onChange={(value) => setOther(value)} />
    </>
  );
}
