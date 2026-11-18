import { createSignal } from "solid-js";

// ADR 1091: a helper `const` of a child component's body, read only in a JSX child, while a tag in the same file renders the
// component. The tag hands the component to Solid's `createComponent`, which runs it during the render that holds the tag (as a
// tag in another file does): the helper's read is the render's, proved.
function Content() {
  const [count] = createSignal(0);
  const shown = () => count() > 0;
  return <p>{shown() ? "yes" : "no"}</p>;
}

// Kept: the component is also copied into a `const` and called through the copy, which nothing here follows.
function Copied() {
  const [count] = createSignal(0);
  const shown = () => count() > 0;
  return <p>{shown() ? "yes" : "no"}</p>;
}
const copy = Copied;
export function CallsCopy() {
  return copy();
}

// Kept: the component is also handed away in an exported array.
function Listed() {
  const [count] = createSignal(0);
  const shown = () => count() > 0;
  return <p>{shown() ? "yes" : "no"}</p>;
}
export const listed = [Listed];

// Kept: the helper read in the component's body runs untracked (SC1001 violation).
function BodyRead() {
  const [count] = createSignal(0);
  const shown = () => count() > 0;
  const first = shown();
  return <p>{first ? "yes" : "no"}</p>;
}

export function Page() {
  return (
    <div>
      <Content />
      <Copied />
      <Listed />
      <BodyRead />
    </div>
  );
}
