import { createStore } from "solid-js";
import { isServer } from "@solidjs/web";
import * as web from "@solidjs/web";

const [st] = createStore({ v: 0, then: 0, items: [0] });

// Only `isServer` renders these: a client build never runs them, proved silent.
const ServerTag = () => { const { v } = st; return <i>{v}</i>; };
const ServerQuoted = () => { const { "v": q } = st; return <i>{q}</i>; };
const ServerNested = () => { const { items: [] } = st; return <i />; };
const ServerThenFirst = () => { const { then, v } = st; return <i>{then}{v}</i>; };
// Namespace import: the same arm.
const NamespaceTag = () => { const { v } = st; return <i>{v}</i>; };

// A second render outside any arm: the claim stands.
const BothArms = () => { const { v } = st; return <i>{v}</i>; };

// `typeof window` is decided by no selection here: open.
const UnderConstant = () => { const { v } = st; return <i>{v}</i>; };

// A local `isServer` is no build constant: the claim stands.
const ShadowedTag = () => { const { v } = st; return <i>{v}</i>; };

// A props destructure is a pattern rule, not a client claim: it stands in the arm.
const ServerProps = ({ n }: { n: number }) => <i>{n}</i>;

function Shadowed() {
  const isServer = Math.random() < 2;
  return <p>{isServer ? <ShadowedTag /> : null}</p>;
}

export function App() {
  return (
    <main>
      <p>{isServer ? <ServerTag /> : null}</p>
      <p>{isServer ? <ServerQuoted /> : null}</p>
      <p>{isServer ? <ServerNested /> : null}</p>
      <p>{isServer ? <ServerThenFirst /> : null}</p>
      <p>{web.isServer ? <NamespaceTag /> : null}</p>
      <p>{isServer ? <BothArms /> : <BothArms />}</p>
      <p>{typeof window === "undefined" ? <UnderConstant /> : null}</p>
      <p>{isServer ? <ServerProps n={st.v} /> : null}</p>
      <Shadowed />
    </main>
  );
}
