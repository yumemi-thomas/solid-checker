import { createEffect, createMemo } from "solid-js";

// Uncertifiable (SC1001 stays): the body reads a member, which may be a
// getter that runs code.
export function C(props: { size: number }) {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  const size = props.size;
  void size;
  return <div />;
}
