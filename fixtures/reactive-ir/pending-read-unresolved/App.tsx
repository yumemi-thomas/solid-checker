import { createMemo, flush, Loading } from "solid-js";

// ADR 0926: a read of a pending source that an SC9012 row holds (a value no
// row follows to the read). SC5003's and SC5008's codes have no position of
// their own, so each rule names its own uncertifiable at the read, blocker
// `pending-read-unresolved`, beside the SC9012.

// A computed key: the destructured name is the memo, and nothing follows it.
export function ComputedKeyRead() {
  const pending = createMemo(async () => 1);
  const { use } = { ["use"]: pending };
  return (
    <p>
      {(() => {
        use();
        return 1;
      })()}
    </p>
  );
}

// An alias as a bare child: the element inserts and calls it.
export function BareChildAlias() {
  const pending = createMemo(async () => 1);
  const use = pending;
  return (
    <p>
      {(() => {
        void (<b>{use as unknown as string}</b>);
        return 1;
      })()}
    </p>
  );
}

// A memo a local hook creates over the pending source, held in an object.
export function HookMemberRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const useM = (g: () => unknown) => createMemo(() => g());
        const box = { m: useM(pending) };
        void box.m();
        return 1;
      })()}
    </p>
  );
}

// Handed to a local function that calls its parameter, from a closure.
export function HandedParameterRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const run = () => {
          const peek = (g: () => unknown) => g();
          void peek(pending);
        };
        run();
        return 1;
      })()}
    </p>
  );
}

// SC5008: the same unfollowed read inside a `try` that swallows its throw,
// in a component run through `forEach`.
export function CaughtNestedRead() {
  const pending = createMemo(async () => 1);
  const { a: { use } } = { a: { use: pending } };
  const K1 = () => {
    flush(() => {
      try {
        use();
      } catch {}
    });
    return <span />;
  };
  return (
    <p>
      {(() => {
        [1].forEach(K1);
        return 1;
      })()}
    </p>
  );
}

// Negative: the source is synchronous, so no pending rule asks.
export function SyncComputedKeyRead() {
  const settled = createMemo(() => 1);
  const { use } = { ["use"]: settled };
  return (
    <p>
      {(() => {
        use();
        return 1;
      })()}
    </p>
  );
}

// Negative: a `Loading` boundary covers the read, so SC5003 is ruled out
// (the SC9012 at the bare child stands).
export function CoveredBareChildAlias() {
  const pending = createMemo(async () => 1);
  const use = pending;
  return (
    <Loading fallback={0}>
      <p>
        {(() => {
          void (<b>{use as unknown as string}</b>);
          return 1;
        })()}
      </p>
    </Loading>
  );
}
