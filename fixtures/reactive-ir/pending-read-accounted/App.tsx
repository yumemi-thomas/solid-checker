import { createEffect, createMemo, isPending, Loading, onSettled } from "solid-js";

// ADR 0838: every pending read is accounted for. Each positive is a read an
// async walk ended at with no row of its rule; each now has an uncertifiable
// row, blocker `async-read-unaccounted`.

// One row per call: `both()` took `first`'s row, and `second`'s read in the
// same helper had none.
export function TwoReadsOneCall() {
  const first = createMemo(async () => 1);
  const second = createMemo(async () => 2);
  const both = () => [first(), second()];
  return <p>{both().length}</p>;
}

// A helper whose binding is written: the walk climbs no written binding.
export function RewrittenHelper(props: { off: boolean }) {
  const pending = createMemo(async () => 1);
  let use = () => pending();
  if (props.off) use = () => Promise.resolve(0);
  return <p>{String(use())}</p>;
}

// A generator helper stepped in a tracked insert: the walk skips generators.
export function GeneratorHelper() {
  const pending = createMemo(async () => 1);
  function* steps() {
    yield pending();
  }
  return <p>{[...steps()].length}</p>;
}

// Review R1/R6: one call placed (tracked, under `<Loading>`), another path cut
// (five closures past the walk's bound in `onSettled`, Chrome rc.21:
// PENDING_ASYNC_FORBIDDEN_SCOPE). The placed call's row speaks for its own
// path only: the read keeps rows of its own.
export function MixedBoundPath() {
  const pending = createMemo(async () => 1);
  const wrapUse = () => {
    pending();
  };
  const c1 = () => wrapUse();
  const c2 = () => c1();
  const c3 = () => c2();
  const c4 = () => c3();
  const c5 = () => c4();
  onSettled(() => {
    c5();
  });
  return (
    <Loading fallback={0}>
      <p>{(wrapUse(), "x")}</p>
    </Loading>
  );
}

// Review R6's twin: one tracked caller under `<Loading>`, one effect-apply
// caller the walk places nowhere.
export function TwoCallers() {
  const pending = createMemo(async () => 1);
  const wrapUse = () => {
    pending();
  };
  createEffect(
    () => 1,
    () => {
      wrapUse();
    }
  );
  return (
    <Loading fallback={0}>
      <p>{(wrapUse(), "x")}</p>
    </Loading>
  );
}

// Negatives, each accounted for: a handler's read (no strict window, no
// render effect), a covered read, a covered probe (a probe's first load
// throws to the boundary around it, review R18), a computation's own read.
export function HandlerRead() {
  const pending = createMemo(async () => 1);
  return <button onClick={() => pending()}>x</button>;
}

export function CoveredRead() {
  const pending = createMemo(async () => 1);
  return (
    <Loading fallback={0}>
      <p>{String(pending())}</p>
    </Loading>
  );
}

export function ProbeRead() {
  const pending = createMemo(async () => 1);
  return (
    <Loading fallback={0}>
      <p>{isPending(() => pending()) ? 1 : 2}</p>
    </Loading>
  );
}

export function ComputationRead() {
  const pending = createMemo(async () => 1);
  const later = createMemo(() => String(pending()));
  return (
    <Loading fallback={0}>
      <p>{later()}</p>
    </Loading>
  );
}
