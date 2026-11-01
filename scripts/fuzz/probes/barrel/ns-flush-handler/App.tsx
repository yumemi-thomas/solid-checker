// Legal control: flush() in a click handler through the barrel.
import * as S from "./helpers";

export default function App() {
  return <button id="w" onClick={() => S.flush()}>go</button>;
}
