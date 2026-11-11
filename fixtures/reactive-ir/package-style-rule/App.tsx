import { Competing, Fine } from "dep-style";

export function Own(props: { html: string }) {
  return <div innerHTML={props.html}>fallback</div>;
}
export function Packaged() {
  return <Competing html="<b>x</b>" />;
}
export function Quiet() {
  return <Fine text="x" />;
}
