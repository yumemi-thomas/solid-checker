import "fz-pub";
function Wrap() {
  return <button onClick={() => __fzAct(() => {})}>go</button>;
}
export function App() {
  return <Wrap />;
}
export async function after() {
  (document.querySelector("button") as HTMLButtonElement).click();
}
