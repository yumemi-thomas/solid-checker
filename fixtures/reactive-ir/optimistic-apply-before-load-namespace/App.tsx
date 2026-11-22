import * as S from "solid-js";

// The same write through a namespace import (Chrome throws): ADR 0505's body premise reads no member, so the namespace's
// `S.createOptimistic` and `S.createEffect` leave it unproven. A stated gap: uncertifiable, never clean.
export function App() {
  const [, setO] = S.createOptimistic(async () => 1);
  S.createEffect(
    () => 1,
    () => {
      setO(2);
    },
  );
  return <div />;
}
