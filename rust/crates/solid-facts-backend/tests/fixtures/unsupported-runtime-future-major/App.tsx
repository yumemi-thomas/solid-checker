// The same shape as `unsupported-runtime-v1/App.tsx`, for the same reason:
// `Greeting` destructures its props, which the Solid 2 catalog reports as
// SC1003 `no-destructure`. So a refusal that reported only SC9013 on a file
// with nothing in it would prove nothing. Here, a build that analyzed this
// tree would say SC1003, and the build that refuses it says SC9013 alone.
//
// What is different is the installed version: `3.0.0` is a major no `Version`
// variant names at all, where `1.9.14` is one this build still recognises in
// order to refuse it. Both must refuse. If `for_solid_js` ever went back to
// answering `None` for an unnamed major, this project would be analyzed as
// Solid 2 in silence and this test is what says so.
import { createSignal } from "solid-js";

export function Greeting({ name }: { name: string }) {
  return <span>{name}</span>;
}

export const App = () => {
  const [name] = createSignal("world");
  return <Greeting name={name()} />;
};
