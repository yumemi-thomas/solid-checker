// The source exists so the refusal has something to suppress.
//
// `Greeting` destructures its props, which both catalogs report as SC1003
// `no-destructure`. That is the point: a refusal that reported *only* SC9013
// on a file with no findings in it would prove nothing about whether the
// refusal replaces the analysis or merely precedes it. Here, every build that
// analyzes this tree reports SC1003, and the build that refuses it reports
// SC9013 and nothing else.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
