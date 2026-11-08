import { shared } from "./listeners";
export function App() {
  window.dispatchEvent(new Event("click"));
  return <button onClick={shared}>go</button>;
}
