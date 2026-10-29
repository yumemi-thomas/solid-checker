import { createEffect, createMemo } from "solid-js";

// Violation (SC5001, ADR 0505 through ADR 0507): a user effect's first apply
// in the first render's flush, in a child an inert parent renders.
function Child() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return <div />;
}

export function Parent() {
  return (
    <div>
      <Child />
    </div>
  );
}
