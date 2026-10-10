// The notice accompanies the analysis instead of replacing it. `Greeting`
// destructures its props, so SC1003 must be reported beside SC9014: seeing
// SC9014 alone would mean the reviewed release had been refused, and seeing
// SC1003 alone would mean the older release had gone unmentioned. The rc.9
// twin of this fixture is the audited release, and carries SC1003 alone.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
