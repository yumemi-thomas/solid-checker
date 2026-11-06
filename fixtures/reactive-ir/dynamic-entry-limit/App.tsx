// ADR 0695: `dynamic_component_entry` climbs 32 callbacks that keep the read
// window. Inside, the body is proven dynamic-only (ADR 0530) and the read is the
// no-window snapshot; past it, nothing named the entry, so the read is
// uncertifiable. Before this, past the bound it kept the rendering role, whose
// hint promised a warning: Chrome is quiet at 33 levels and at 40.
import { createSignal, dynamicComponent, flush } from "solid-js";

const [c] = createSignal(1);

// Violation, no-window hint: thirty levels.
function Inside() {
  flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { void c(); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); });
  return <div />;
}

// Uncertifiable: thirty-three levels.
function Past() {
  flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { flush(() => { void c(); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); });
  return <div />;
}

export const D1 = dynamicComponent(() => Inside);
export const D2 = dynamicComponent(() => Past);

export function App() {
  return (
    <div>
      <D1 />
      <D2 />
    </div>
  );
}
