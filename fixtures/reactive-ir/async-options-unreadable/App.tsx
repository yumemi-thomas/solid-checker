import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

declare const opaqueOptions: { loadingValue?: { name: string } };

// Options the analysis cannot read may declare `loadingValue`, whose first
// flight serves the declared value and needs no boundary: the bare render is
// uncertifiable, never a violation (ADR 0388).
const variableUser = createMemo(async () => ({ name: "Ada" }), opaqueOptions);
const spreadUser = createMemo(async () => ({ name: "Grace" }), { ...opaqueOptions });

function OpaquePage() {
  return <div>{variableUser().name}{spreadUser().name}</div>;
}

render(() => <OpaquePage />, document.body);

// Readable literal options without the key leave the bare render a violation,
// as does a memo with no options at all.
const namedUser = createMemo(async () => ({ name: "Dennis" }), { name: "named" });
const bareUser = createMemo(async () => ({ name: "Ken" }));

function LiteralPage() {
  return <div>{namedUser().name}{bareUser().name}</div>;
}

render(() => <LiteralPage />, document.body);

// A readable literal that declares `loadingValue` is the declared-first-paint
// exception: no finding.
const declaredUser = createMemo(async () => ({ name: "Barbara" }), { loadingValue: { name: "" } });

function DeclaredPage() {
  return <div>{declaredUser().name}</div>;
}

render(() => <DeclaredPage />, document.body);
