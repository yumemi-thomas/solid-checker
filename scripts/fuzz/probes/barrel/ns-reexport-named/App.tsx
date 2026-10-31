import * as S from "./helpers";

export default function App() {
  const [r] = S.createSignal(1);
  const seen = r();
  return <span id="w">{seen}</span>;
}
