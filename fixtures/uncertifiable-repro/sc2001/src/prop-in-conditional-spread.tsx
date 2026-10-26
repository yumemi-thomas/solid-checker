// kui feature-connect ConnectorList.tsx:167-172 (callback from ConnectRoute.tsx:180): the child spreads an object holding a closure that
// calls the callback prop onto a component, `{...(props.onCommand === undefined ? {} : { onCommand: ... })}`.
import { createSignal } from "solid-js";

type Command = "pause" | "resume";

function Row(props: { name: string; onCommand?: (which: Command) => void }) {
  return <button onClick={() => props.onCommand?.("pause")}>{props.name}</button>;
}

function List(props: { onCommand?: (name: string, which: Command) => void }) {
  return (
    <Row
      name="a"
      {...(props.onCommand === undefined ? {} : { onCommand: (which: Command) => props.onCommand?.("a", which) })}
    />
  );
}

export function Page() {
  const [pending, setPending] = createSignal("");
  const onCommand = (name: string, which: Command): void => {
    setPending(`${name} ${which}`);
  };
  return (
    <>
      <List onCommand={onCommand} />
      <p>{pending()}</p>
    </>
  );
}
