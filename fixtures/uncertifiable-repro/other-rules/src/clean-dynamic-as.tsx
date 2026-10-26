// Clean. Mirrors hoxxes-briefing apps/web/src/shared/ui/eyebrow/index.tsx:33-41: a Dynamic component chosen by
// an optional `as` prop with a default tag, forwarding class and children.
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";

function Eyebrow(props: { as?: "p" | "span"; class?: string; children?: JSX.Element }) {
  return (
    <Dynamic component={props.as ?? "p"} class={props.class}>
      {props.children}
    </Dynamic>
  );
}

export function App() {
  return (
    <>
      <Eyebrow>one</Eyebrow>
      <Eyebrow as="span" class="tone">two</Eyebrow>
    </>
  );
}
