import { createEffect, createRoot, createSignal, flush } from "solid-js";

createRoot(() => {
  const [value] = createSignal(1);

  // Negative: a Proxy built from a literal whose `get` trap cannot return, read
  // before the call.
  createEffect(
    () => 1,
    () => {
      const box = new Proxy(
        { value: 1 },
        {
          get(): never {
            throw new Error("proxy get");
          },
        },
      );
      box.value;
      flush();
    },
  );

  // Negative: the same for a `set` trap and a write.
  createEffect(
    () => 1,
    () => {
      const box = new Proxy(
        { value: 1 },
        {
          set(): boolean {
            throw new Error("proxy set");
          },
        },
      );
      box.value = 2;
      flush();
    },
  );

  // Positive: a trap that returns completes normally, and so does a read of an
  // object that is not the proxy (a name is matched, so the positives use their own).
  createEffect(value, () => {
    const forwarding = new Proxy(
      { value: 1 },
      {
        get(target, key): unknown {
          return (target as Record<PropertyKey, unknown>)[key];
        },
      },
    );
    forwarding.value;
    flush();
  });
  createEffect(value, (current) => {
    const plain = { current };
    plain.current;
    flush();
  });
});
