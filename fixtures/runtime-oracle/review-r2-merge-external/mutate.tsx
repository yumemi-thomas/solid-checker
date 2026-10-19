import type { JSX } from "@solidjs/web";
export function replace(props: { children: () => JSX.Element }) {
  props.children = () => <span>replacement</span>;
}
