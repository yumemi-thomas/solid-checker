// Clean. Mirrors jar-hell-web web/src/pages/PackagePage.tsx:22-41,56-63: a writable derived signal, an async
// function that writes it after an await only if `latest(gav)` still matches, called from an effect apply.
import { createEffect, createMemo, createSignal, latest, Loading, Show } from "solid-js";

const getPackage = async (id: string) => ({ id, name: `${id}-old` });
const analyzePackage = async (id: string) => ({ id, name: `${id}-new` });

export function Page() {
  const [coordinate, setCoordinate] = createSignal("a:b");
  const gav = createMemo(() => coordinate().trim());
  const analyzed = createMemo(() => gav().length > 3);
  const [pkg, setPkg] = createSignal(() => ((gav(), analyzed()) ? getPackage(gav()) : undefined));
  const analyze = async (g: string) => {
    const data = await analyzePackage(g);
    if (g === latest(gav)) setPkg(data);
  };
  createEffect(
    () => (analyzed() ? undefined : gav()),
    (g) => {
      if (g) analyze(g);
    },
  );
  return (
    <Loading on={analyzed} fallback={<p>loading</p>}>
      <button type="button" onClick={() => setCoordinate(coordinate() + "x")}>grow</button>
      <Show when={pkg()}>{(p) => <p>{p().name}</p>}</Show>
    </Loading>
  );
}
