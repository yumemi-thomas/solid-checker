import { record, relay } from "./helpers";

function Label(props: { text: string }) {
  const direct = record(props.text);
  const relayed = relay(props.text);
  return (
    <main>
      {String(direct)}
      {String(relayed)}
    </main>
  );
}

// The one tag that renders `Label` passes a string, so `props.text` is a primitive on every run.
export function Page() {
  return <Label text="one" />;
}
