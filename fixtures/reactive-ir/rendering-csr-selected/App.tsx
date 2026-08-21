// One claim, two consumers: an explicit client-only rendering selector
// *proves* the server-rendering premise false, and a rule whose whole claim
// is conditioned on "if this application server-renders" must then be silent
// rather than report a proof obligation the user has already discharged.
//
// The selector lives in .solid-checker/runtime.json (rendering: "csr"). The
// paired fixtures cover the other two states of the same fact:
//   ssr-client-boundary       -- server rendering proven  -> violation
//   ssr-client-boundary-csr   -- premise unresolved       -> uncertifiable
//   http-response-flush-csr   -- premise unresolved       -> uncertifiable
//
// Absence of a visible server entry is not what makes this fixture quiet;
// ssr-client-boundary-csr has no server entry either and still reports. The
// selector is.
import { Loading, createMemo } from "solid-js";
import { httpHeader, httpStatus } from "@solidjs/web";

// SC5003's ssrSource arm: a bare client source is a server-render hole only
// where the server runs. Under proven CSR there is no server render, so the
// hole cannot exist and no read of it is reported.
const widget = createMemo(() => computeWidth(), { ssrSource: "client" });

export function QuietWidget() {
  return <div>{widget()}</div>;
}

// SC7005: the drop is the SSR shell flush committing the response head.
// Under proven CSR there is no shell and no response head, so there is
// nothing to drop.
const product = createMemo(() => fetchProduct());

export function QuietResponseHead() {
  httpStatus(410);
  httpHeader("cache-control", "no-store");
  return <div>{product().name}</div>;
}

export function Page() {
  return (
    <Loading fallback={<div />}>
      <QuietResponseHead />
    </Loading>
  );
}

// The positive control, and the reason this fixture cannot pass by being
// empty of analyzable code: SC5003's *async* arm does not depend on the
// rendering premise at all. A pending async accessor rendered with no
// Loading boundary above it still shows nothing while loading under CSR, so
// this one is still reported.
const profile = createMemo(async () => loadProfile());

export function LoudProfile() {
  return <div>{profile().name}</div>;
}

declare function computeWidth(): number;
declare function fetchProduct(): { available: boolean; name: string };
declare function loadProfile(): Promise<{ name: string }>;
