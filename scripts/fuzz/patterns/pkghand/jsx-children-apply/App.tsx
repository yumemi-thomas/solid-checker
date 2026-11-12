import { flush } from "solid-js";
import { ApplyView } from "fz-step";
export function App() {
  setTimeout(() => {
    document.body.append((<ApplyView>{() => { flush(); }}</ApplyView>) as unknown as Node);
  });
  return <p>x</p>;
}
