import { createEffect, createMemo, createSignal, createTrackedEffect, onSettled } from "solid-js";
import { Cleans, Reads, Takes, creates, flushes, makeCreates, makeFlushes, quiet, reads, tag } from "dep-ops";

// A leaf owner's callback (SC3001). The call walk follows an exact call into a plain function; a constructor and a
// returned object's getter it does not enter.
export function LeafNew() {
  onSettled(() => {
    new Cleans();
  });
  return <p>x</p>;
}
export function LeafGetterCreates() {
  createTrackedEffect(() => {
    makeCreates().value;
  });
  return <p>x</p>;
}
export function LeafGetterFlushes() {
  createTrackedEffect(() => {
    makeFlushes().value;
  });
  return <p>x</p>;
}
export function LeafCall() {
  onSettled(() => {
    flushes();
  });
  return <p>x</p>;
}
export function LeafQuiet() {
  onSettled(() => {
    quiet();
  });
  return <p>x</p>;
}
export function BodyNew() {
  new Cleans();
  return <p>x</p>;
}

// An effect's apply callback (SC2008, SC6002): no walk runs from the apply down into a call.
export function ApplyFlush() {
  createEffect(
    () => 1,
    () => {
      flushes();
    },
  );
  return <p>x</p>;
}
export function ApplyCreate() {
  createEffect(
    () => 1,
    () => {
      creates();
    },
  );
  return <p>x</p>;
}
export function ApplyQuiet() {
  createEffect(
    () => 1,
    () => {
      quiet();
    },
  );
  return <p>x</p>;
}

// After the await of an async tracked computation (SC1002).
export function AwaitNew() {
  createMemo(async () => {
    await Promise.resolve();
    new Reads();
    return 1;
  });
  return <p>x</p>;
}
export function AwaitTag() {
  createMemo(async () => {
    await Promise.resolve();
    tag`x`;
    return 1;
  });
  return <p>x</p>;
}
export function AwaitHanded() {
  const [n] = createSignal(0);
  createMemo(async () => {
    await Promise.resolve();
    new Takes(n);
    return 1;
  });
  return <p>x</p>;
}
export function AwaitCall() {
  createMemo(async () => {
    await Promise.resolve();
    reads();
    return 1;
  });
  return <p>x</p>;
}
export function BeforeAwait() {
  createMemo(async () => {
    new Reads();
    await Promise.resolve();
    return 1;
  });
  return <p>x</p>;
}

// An async accessor handed to a package (SC5001).
export function AsyncHanded() {
  const later = createMemo(async () => 1);
  new Takes(later);
  return <p>x</p>;
}
