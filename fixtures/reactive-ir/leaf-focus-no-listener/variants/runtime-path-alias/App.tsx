import { onCleanup, onSettled } from "solid-js";
import { addEvent } from "@web/web";

// ADR 1148 r1b's lesson: the call is the quiet one (`click`), which the dialect exempts when `addEvent` is imported by the runtime's own specifier. Here a `paths` alias
// reaches the runtime's file by a name no audit lists, so the build's attested resolution says where it lands: a file of `@solidjs/web`, unknown, taken as registering.
const h = () => {
  onCleanup(() => {});
};
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    addEvent(el, "click", h, false);
  }} />;
}
