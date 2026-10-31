// ADR 0517: flush() in an effect apply through a project barrel's namespace (FLUSH_IN_EFFECT_CALLBACK).
import * as S from "./helpers";

export default function App() {
  S.createEffect(() => 1, () => { S.flush(); });
  return <span id="w">ok</span>;
}
