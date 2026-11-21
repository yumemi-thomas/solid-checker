import * as S from "solid-js";

// ADR 1175: a render effect's apply runs at its creation's flush, and the store's seed is no answer: a violation, through a
// namespace import.
export function App() {
  const [, setO] = S.createOptimisticStore(async () => ({ v: 1 }), { v: 0 });
  S.createRenderEffect(
    () => 1,
    () => {
      setO((draft) => {
        draft.v = 2;
      });
    },
  );
  return <div />;
}
