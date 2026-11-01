// A namespace of a project module that `export *`s Solid (STRICT_READ_UNTRACKED).
import * as S from "./helpers";

export default function App() {
  const [r] = S.createSignal(1);
  const seen = r();
  return <span id="w">{seen}</span>;
}
