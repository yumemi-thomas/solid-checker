// app-game solid-dnd-playground App.tsx:27,39-40: a local component rendered by a JSX tag inside a package callback.
import { Errored } from "solid-js";

function DemoError(props: { message: string }) {
  console.error("Demo error:", props.message);
  return <pre>{props.message}</pre>;
}

export function App() {
  return <Errored fallback={(err) => <DemoError message={String(err())} />}>ok</Errored>;
}
