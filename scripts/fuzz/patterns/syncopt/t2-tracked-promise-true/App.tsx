import { createTrackedEffect, Loading } from "solid-js";
function Inner() {
  try {
    createTrackedEffect((() => Promise.resolve(1)) as any, { sync: true });
  } catch (e) {}
  return <i>app</i>;
}
export function App() {
  return <Loading fallback="..."><Inner /></Loading>;
}
