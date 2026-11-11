import * as S from "solid-js";

const [n] = S.createSignal(0);

function make(skip: boolean) {
  if (skip) return;
  S.createMemo(() => 1);
}
function flushes() {
  S.flush();
}

// The namespace spelling of the layers and of the primitive a helper creates, entered twice under different contexts (ADR 0961).
export function TwiceForbidden() {
  S.createTrackedEffect(() => {
    n();
    make(true);
    make(false);
  });
  return <p>x</p>;
}
export function SwallowedThenDirect() {
  S.createTrackedEffect(() => {
    n();
    S.isPending(flushes);
    flushes();
  });
  return <p>x</p>;
}
// Entered only the tolerant way: clean.
export function TwiceSkipped() {
  S.createTrackedEffect(() => {
    n();
    make(true);
    make(true);
  });
  return <p>x</p>;
}
export function SwallowedOnly() {
  S.createTrackedEffect(() => {
    n();
    S.isPending(flushes);
  });
  return <p>x</p>;
}
