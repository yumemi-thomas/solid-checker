import * as S from "./helpers";

export default function App() {
  const [v, setV] = S.createSignal(0);
  S.createMemo(() => { setV(1); return 1; });
  return <span id="w">{v()}</span>;
}
