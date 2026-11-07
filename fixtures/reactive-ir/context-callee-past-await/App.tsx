import { createContext, createMemo, useContext } from "solid-js";

// ADR 0888: a call in the rest of a block an await of its own async function
// dominates runs on that function's continuation, where no owner is current,
// so the callee gains no owned run from it.
const NoDefault = createContext<number>();

declare const flag: boolean;

// SC4001 violation at the read (NoOwnerError): `f1` runs only from the memo's
// continuation, before its own await too. No owned run, so no SC4002.
export function PastAwaitCallee() {
  const f1 = async () => {
    useContext(NoDefault);
    await Promise.resolve();
    return 1;
  };
  const m1 = createMemo(async () => {
    await Promise.resolve();
    await f1();
    return 1;
  });
  return <p>{String(m1())}</p>;
}

// SC4002 violation: `f2` is called before the memo's await, under its owner,
// with no provider above (ContextNotFoundError).
export function BeforeAwaitCallee() {
  const f2 = async () => {
    useContext(NoDefault);
    return 1;
  };
  const m2 = createMemo(async () => {
    await f2();
    await Promise.resolve();
    return 1;
  });
  return <p>{String(m2())}</p>;
}

// SC4002 violation: the await is skippable, so `f3` may run under the memo's
// owner (a branch the checker cannot evaluate is reachable).
export function SkippableAwaitCallee() {
  const f3 = async () => {
    useContext(NoDefault);
    return 1;
  };
  const m3 = createMemo(async () => {
    if (flag) await Promise.resolve();
    await f3();
    return 1;
  });
  return <p>{String(m3())}</p>;
}

// SC4001 violations: a sync helper called twice, both past the await.
export function SyncCalleePastAwait() {
  const h4 = () => {
    useContext(NoDefault);
  };
  const m4 = createMemo(async () => {
    await Promise.resolve();
    h4();
    h4();
    return 1;
  });
  return <p>{String(m4())}</p>;
}
