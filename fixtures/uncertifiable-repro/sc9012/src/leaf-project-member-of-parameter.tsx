// donegeon-client BoardControllerSummary.tsx:317: createTrackedEffect calls a member of a parameter typed by a
// project type literal, so the caller-supplied function behind it is not known.
import { createSignal, createTrackedEffect } from "solid-js";

type Toast = { error: (message: string) => void };

export function createFailureToast(toast: Toast) {
  const [failure, setFailure] = createSignal("");
  createTrackedEffect(() => {
    const message = failure();
    if (message) toast.error(message);
  });
  return setFailure;
}
