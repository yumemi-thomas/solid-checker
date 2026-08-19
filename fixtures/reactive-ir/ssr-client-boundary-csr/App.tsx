import { createMemo } from "solid-js";

// The same bare ssrSource: "client" source as ssr-client-boundary. This
// analyzed project has no server-render import, but that does not prove the
// application is CSR-only: its server entry may live in another tsconfig or
// package. SC5005 must therefore be uncertifiable rather than silent.
const widget = createMemo(() => computeWidth(), { ssrSource: "client" });

export function CsrWidget() {
  return <div>{widget()}</div>;
}

declare function computeWidth(): number;
