// app-game math-examples m2x3/demo-rotation-matrix-between-point-pairs.tsx:73: a JSX attribute calls
// an element of an array of accessors the component builds itself (`points[0]()`).
import { createSignal } from "solid-js";

export function Marker() {
  const points = [createSignal(0), createSignal(1)].map(([point]) => point);
  return <line x1={points[0]()} stroke="black" />;
}
