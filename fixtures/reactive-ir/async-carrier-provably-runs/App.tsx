import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

// ADR 0563: a pending async read in a callback a standard-library carrier runs is proven (SC5001,
// SC5003) only where the carrier provably runs it.

// Violation (SC5001 at the read): non-empty literal receivers.
export function BodyLiteralNonEmpty() {
  const user = createMemo(async () => 1);
  [1].forEach(() => {
    user();
  });
  return <p />;
}

export function BodySortTwo() {
  const user = createMemo(async () => 1);
  const hf = (_a: number, _b: number) => {
    user();
    return 0;
  };
  [2, 1].sort(hf);
  return <p />;
}

export function BodyFromLiteral() {
  const user = createMemo(async () => 1);
  const hf = () => {
    user();
  };
  Array.from([1], hf);
  return <p />;
}

// Uncertifiable, never a violation: an empty or unknown-length receiver may run the callback zero
// times (rc.17 reads nothing and throws nothing there).
export function BodyEmptyNamed() {
  const user = createMemo(async () => 1);
  const hf = () => {
    user();
  };
  ([] as number[]).forEach(hf);
  return <p />;
}

export function BodyPropsReceiver(props: { xs: number[] }) {
  const user = createMemo(async () => 1);
  const hf = () => {
    user();
  };
  props.xs.forEach(hf);
  return <p />;
}

export function BodyEmptyLiteral() {
  const user = createMemo(async () => 1);
  ([] as number[]).forEach(() => {
    user();
  });
  return <p />;
}

export function BodySetForEach() {
  const user = createMemo(async () => 1);
  new Set([1]).forEach(() => {
    user();
  });
  return <p />;
}

export function BodySortOne() {
  const user = createMemo(async () => 1);
  const hf = (_a: number, _b: number) => {
    user();
    return 0;
  };
  [1].sort(hf);
  return <p />;
}

// SC5003 in rendered JSX (both mounted below, no Loading boundary): a non-empty receiver reads the
// pending source while it renders (violation); an empty one reads nothing (uncertifiable).
export function JsxNonEmptyMap() {
  const user = createMemo(async () => 1);
  return <div>{[1].map(() => user())}</div>;
}

export function JsxEmptyMap() {
  const user = createMemo(async () => 1);
  return <div>{([] as number[]).map(() => user())}</div>;
}

render(() => <JsxNonEmptyMap />, document.body);
render(() => <JsxEmptyMap />, document.body);
