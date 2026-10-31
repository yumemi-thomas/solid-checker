// Legal twin: the read is tracked by JSX.
import * as S from "./helpers";

export default function App() {
  const [r] = S.createSignal(1);
  return <span id="w">{r()}</span>;
}
