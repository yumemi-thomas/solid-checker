import { createMemo, createProjection, Loading } from "solid-js";
import { renderToStream } from "@solidjs/web";

// A bare ssrSource: "client" source: the server never runs the compute and
// nothing is declared to render in its place, so any SSR read outside a
// Loading boundary throws — even though this compute is fully synchronous
// (no async provenance; SC5001–SC5003 can never see it).
const widget = createMemo(() => computeWidth(), { ssrSource: "client" });

// Declared client sources are not holes: the server renders the declared
// value (loadingValue) or the seed (seedLoadingValue: true).
const declaredDraft = createMemo(() => readDraft(), { ssrSource: "client", loadingValue: null });
const seededPanel = createProjection((draft) => { draft.width = computeWidth(); }, { width: 0 }, {
  ssrSource: "client",
  seedLoadingValue: true,
});

export function BadWidget() {
  return <div>{widget()}</div>;
}

export function GoodBoundedWidget() {
  return <Loading fallback={<div />}>{widget()}</Loading>;
}

export function GoodDeclaredDraft() {
  return <div>{declaredDraft()}</div>;
}

export function GoodSeededPanel() {
  return <div>{seededPanel.width}</div>;
}

// This visible server entry proves the violation. Without it (see the
// ssr-client-boundary-csr fixture), SC5005 is uncertifiable rather than safe.
export const server = { render: renderToStream };

declare function computeWidth(): number;
declare function readDraft(): string | null;
