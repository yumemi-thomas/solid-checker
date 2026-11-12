import { record, relay } from "./helpers";

function Label(props: { text: unknown }) {
  const direct = record(props.text);
  const relayed = relay(props.text);
  return (
    <main>
      {String(direct)}
      {String(relayed)}
    </main>
  );
}

// A second tag passes an object: `props.text` is no longer a primitive on every run.
export function Page() {
  return (
    <div>
      <Label text="one" />
      <Label text={{ x: 1 }} />
    </div>
  );
}
