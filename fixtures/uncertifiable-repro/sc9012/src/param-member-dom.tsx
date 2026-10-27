// ai-memory-ui markdown.tsx:378-383: an effect's apply hands the element its
// compute returned to a helper calling a DOM member on it.
import { createEffect, createSignal, onCleanup } from "solid-js";

function bindProseScrollAreas(host: HTMLElement): () => void {
  const areas = host.querySelectorAll(".prose-scroll");
  return () => console.debug("unbound", areas.length);
}

export function Markdown(props: { html: string }) {
  const [host, setHost] = createSignal<HTMLElement>();
  let stop: (() => void) | undefined;
  createEffect(
    () => ({ el: host(), markup: props.html }),
    (next) => {
      stop?.();
      stop = next.el ? bindProseScrollAreas(next.el) : undefined;
    },
  );
  onCleanup(() => stop?.());
  return <div ref={setHost} innerHTML={props.html} />;
}
