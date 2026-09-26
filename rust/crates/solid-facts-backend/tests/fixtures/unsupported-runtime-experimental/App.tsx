// The same shape as `unsupported-runtime-v1/App.tsx`, for the same reason:
// `Greeting` destructures its props, which the Solid 2 catalog reports as
// SC1003 `no-destructure`, so SC9013 alone proves the refusal replaced the
// analysis.
//
// What is different is the installed version. `2.0.0-experimental.1` names a
// major this build *carries*, so the major alone would analyze it as Solid 2.
// It is the pre-beta experiment, whose runtime reads other argument positions
// (the rc.9 review, § 8), and the Solid 2 vocabulary refuses the line.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
