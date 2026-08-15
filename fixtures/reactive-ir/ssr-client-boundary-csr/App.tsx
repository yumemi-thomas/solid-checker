import { createMemo } from "solid-js";

// The same bare ssrSource: "client" source as ssr-client-boundary — but this
// project never imports a server rendering entry point, so the server code
// path that throws can never run and SC5005 must stay silent (firing on a
// CSR-only app would itself be a false positive).
const widget = createMemo(() => computeWidth(), { ssrSource: "client" });

export function CsrWidget() {
  return <div>{widget()}</div>;
}

declare function computeWidth(): number;
