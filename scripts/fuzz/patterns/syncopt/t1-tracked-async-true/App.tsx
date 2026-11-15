import { createTrackedEffect, Loading } from "solid-js";
function Inner() {
  try {
    createTrackedEffect(async () => { await 0; }, { sync: true });
  } catch (e) {}
  return <i>app</i>;
}
export function App() {
  return <Loading fallback="..."><Inner /></Loading>;
}
