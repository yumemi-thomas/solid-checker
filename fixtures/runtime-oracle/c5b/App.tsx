import { createSignal } from "solid-js";
import * as L from "./lib";

export function Loop() {
  const [count] = createSignal(0);
  const h = L.loopy(() => console.log(count()));
  return <div {...h}>d</div>;
}
export function Helper() {
  const [count] = createSignal(0);
  const h = L.viaHelper(() => console.log(count()));
  return <div {...h}>e</div>;
}
export function Try() {
  const [count] = createSignal(0);
  const h = L.viaTry(() => console.log(count()));
  return <div {...h}>f</div>;
}
export function Iife() {
  const [count] = createSignal(0);
  const h = L.iife(() => console.log(count()));
  return <div {...h}>m</div>;
}
export function ObjectCall() {
  const [count] = createSignal(0);
  const h = L.viaObjectCall(() => console.log(count()));
  return <div {...h}>n</div>;
}
export function Plain() {
  const [count] = createSignal(0);
  const h = L.handlers(() => console.log(count()));
  return <div {...h}>n</div>;
}
