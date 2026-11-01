// Obligation gap candidate: the boundary's children arrive through the `children` attribute, not JSX children.
import { Loading, createMemo, createSignal } from "solid-js";
export default function App() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return (
    <div>
      <Loading fallback={0} on={t()} children={<p>{a()}</p>} />
      <p>{a()}</p>
      <button id="w" onClick={() => setT(2)}>go</button>
    </div>
  );
}
