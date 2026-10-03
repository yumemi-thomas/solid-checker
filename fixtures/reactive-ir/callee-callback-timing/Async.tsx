import { createMemo, isPending } from "solid-js";
import { runNow, runLater } from "./helpers";
import * as helpers from "./helpers";
import * as Solid from "solid-js";

declare const harness: { checkPending(callback: () => unknown): boolean; retained?: () => unknown };

export function UnknownPendingHandler() {
  const result = createMemo(async () => 1);
  harness.checkPending(() => result()); // uncertifiable: pending handling unknown
  return <span />;
}

export function DeferredPendingRead() {
  const result = createMemo(async () => 1);
  runLater(() => result()); // uncertifiable: callback invocation unknown
  return <span />;
}

export function DirectPendingRead() {
  const result = createMemo(async () => 1);
  result(); // violation: pending read in the component body
  return <span />;
}

export function InlinePendingRead() {
  const result = createMemo(async () => 1);
  runNow(() => result()); // violation: exact cross-file helper invokes it
  helpers.runNow(() => result()); // violation: same symbol through namespace
  return <span />;
}

export function NativePendingHandler() {
  const result = Solid.createMemo(async () => 1);
  isPending(() => result()); // clean: known native pending probe
  Solid.isPending(() => result()); // same exact primitive through namespace
  return <span />;
}

export function ShadowedPendingHandler() {
  const result = createMemo(async () => 1);
  const isPending = (callback: () => unknown) => callback();
  isPending(() => result()); // violation: local symbol supplies no pending probe
  return <span />;
}

export function StoredPendingHandler() {
  const result = createMemo(async () => 1);
  const target: { callback?: () => unknown } = {};
  target.callback = () => result(); // uncertifiable: lexical placement does not prove invocation
  return <span />;
}

export function RetainedPendingHandler() {
  const result = createMemo(async () => 1);
  const callback = () => result(); // negative: no proven pending exception in a retained callback
  harness.retained = callback;
  return <span />;
}
