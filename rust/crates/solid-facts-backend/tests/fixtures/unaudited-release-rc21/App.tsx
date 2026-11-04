// The notice accompanies the analysis instead of replacing it. `Greeting`
// destructures its props, so SC1003 must be reported beside SC9014: seeing
// SC9014 alone would mean the reviewed release had been refused, and seeing
// SC1003 alone would mean the unaudited release had gone unmentioned.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
