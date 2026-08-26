import {
  createErrorBoundary,
  createLoadingBoundary,
  createMemo,
  createSignal,
  repeat,
} from "solid-js";

// The node export variants deliberately differ from the browser fixture:
// repeat is inline, both error-boundary callbacks are deferred, and only the
// loading boundary body is inline. Contract timing must override the native
// browser-default table without replacing native ownership or return facts.
export function ServerCallbackVariants() {
  const [count] = createSignal(0);
  createMemo(async () => {
    await Promise.resolve();
    repeat(() => count(), () => count());
    createErrorBoundary(() => count(), () => count());
    createLoadingBoundary(() => count(), () => count());
  });
  return <div />;
}
