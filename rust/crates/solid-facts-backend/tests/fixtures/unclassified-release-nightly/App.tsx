// ADR 0700: a `solid-js` whose version names no release (`nightly`) is
// analyzed under the default dialect and carries the SC9014 notice with the
// `solid-version-unclassified` blocker. `Greeting` destructures its props, so
// SC1003 beside the notice shows the analysis ran; certified would be a clean
// no release ever audited.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
