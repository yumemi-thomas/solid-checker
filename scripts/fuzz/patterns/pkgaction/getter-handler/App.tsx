import { clock } from "fz-get";
function Wrap() {
  return <button onClick={() => void clock.now}>go</button>;
}
export function App() {
  return <Wrap />;
}
export async function after() {
  (document.querySelector("button") as HTMLButtonElement).click();
}
