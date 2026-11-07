import { onSettled } from "solid-js";

export function App() {
  setTimeout(() => {
    (onSettled as any)(() => () => {});
  }, 0);
  setTimeout(() => {
    onSettled(() => {});
  }, 0);
  return <p />;
}
