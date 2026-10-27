// Clean. Mirrors dashiboard-ui dashiboard-ui/src/components/Button.tsx:70: a ref callback reads a prop under
// untrack to hand the element to the prop's own ref function.
import { createSignal, untrack } from "solid-js";

function Button(props: { menu?: { ref: (el: HTMLElement) => void } }) {
  return <button type="button" ref={(el) => untrack(() => props.menu)?.ref(el)}>open</button>;
}

export function App() {
  const [seen] = createSignal("none");
  return (
    <>
      <Button menu={{ ref: (el) => console.log(el.tagName) }} />
      <p>{seen()}</p>
    </>
  );
}
