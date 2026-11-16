import { For, createContext, useContext } from "solid-js";
const Ctx = createContext(7);
function Drop(props: { children?: unknown }) {
  void props;
  return <div></div>;
}
export function App() {
  return <Drop><For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For></Drop>;
}
