import { createSignal } from "solid-js";

// Positive: a synchronous ref callback runs while its element is created, in
// the component body's strict-read window; the IIFE runs on its stack.
export function SyncRefIife() {
  const [r1] = createSignal(3);
  return (
    <div
      ref={(el) => {
        (() => {
          el.textContent = String(r1());
        })();
      }}
    />
  );
}

// Negative (ADR 0938): past the async ref callback's await the IIFE runs on a
// promise continuation, with no window, as the direct read there does.
export function AsyncRefIife() {
  const [r1] = createSignal(3);
  return (
    <div
      ref={async (el) => {
        el.textContent = "pre";
        await Promise.resolve();
        (() => {
          el.textContent = String(r1());
        })();
      }}
    />
  );
}

// Negative: the same through an inline `forEach` callback.
export function AsyncRefForEach() {
  const [r1] = createSignal(3);
  return (
    <div
      ref={async (el) => {
        await Promise.resolve();
        [1].forEach(() => {
          el.textContent = String(r1());
        });
      }}
    />
  );
}

// Negative: a function bound to a name and called once, after the await.
export function AsyncRefNamed() {
  const [r1] = createSignal(3);
  return (
    <div
      ref={async (el) => {
        const show = () => {
          el.textContent = String(r1());
        };
        await Promise.resolve();
        show();
      }}
    />
  );
}

// Negative: an async helper the body calls; its IIFE runs after the helper's
// await, off the body's stack.
export function BodyAsyncHelper() {
  const [r1] = createSignal(3);
  const load = async () => {
    await Promise.resolve();
    (() => {
      console.log(r1());
    })();
  };
  void load();
  return <div />;
}

// Positive: the IIFE runs before the await, still in the window.
export function AsyncRefIifeBeforeAwait() {
  const [r1] = createSignal(3);
  return (
    <div
      ref={async (el) => {
        (() => {
          el.textContent = String(r1());
        })();
        await Promise.resolve();
      }}
    />
  );
}

// Uncertifiable: an await that may be skipped leaves the layer unplaced.
export function AsyncRefSkippableAwait(props: { wait: boolean }) {
  const [r1] = createSignal(3);
  return (
    <div
      ref={async (el) => {
        if (props.wait) await Promise.resolve();
        (() => {
          el.textContent = String(r1());
        })();
      }}
    />
  );
}
