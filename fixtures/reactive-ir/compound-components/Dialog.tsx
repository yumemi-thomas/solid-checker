import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

function Content() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: the holder is exported through a barrel (`index.ts`) and imported
// and tagged in `Screen.tsx`; an export is module surface, as for a plain
// component.
export const Dialog = { Root, Content };
