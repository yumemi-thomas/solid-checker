import { createRenderEffect, Loading } from "solid-js";
function Inner() {
  createRenderEffect(async () => 1, () => {});
  return <i>app</i>;
}
export function App() {
  return <Loading fallback="..."><Inner /></Loading>;
}
