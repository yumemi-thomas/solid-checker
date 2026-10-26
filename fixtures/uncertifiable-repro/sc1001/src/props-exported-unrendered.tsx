// ai-memory-ui sidebar.tsx:50-51: an exported component, rendered nowhere in the project,
// seeds a signal from a prop (a library-style component with no caller to enumerate).
import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

export function SidebarProvider(props: { defaultOpen?: boolean; children: JSX.Element }) {
  const [open, setOpen] = createSignal(props.defaultOpen ?? true);
  return (
    <section data-open={open()}>
      <button onClick={() => setOpen((value) => !value)}>toggle</button>
      {props.children}
    </section>
  );
}
