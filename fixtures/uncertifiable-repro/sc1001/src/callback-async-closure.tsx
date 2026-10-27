// kui feature-messages MessagesRoute.tsx:250 (kernel data/mutation.ts:58): a callback reading props is handed to a project function that
// calls it in an async `run` closure it returns.
import { createSignal } from "solid-js";

function createMutation(call: (name: string) => Promise<string>) {
  const [state, setState] = createSignal("idle");
  const run = async (name: string): Promise<void> => {
    setState("running");
    setState(await call(name));
  };
  return { state, run };
}

export function Resend(props: { topic: string }) {
  const copy = createMutation((name) => Promise.resolve(`${props.topic}/${name}`));
  return <button onClick={() => void copy.run("x")}>{String(copy.state())}</button>;
}
