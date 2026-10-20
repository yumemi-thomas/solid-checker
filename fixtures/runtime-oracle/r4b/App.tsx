import { createSignal } from "solid-js";
import * as L from "./lib";

export function A1() { const [c] = createSignal(0); const h = L.tagged(() => console.log(c())); return <button {...h}>1</button>; }
export function A2() { const [c] = createSignal(0); const h = L.reflectApply(() => console.log(c())); return <button {...h}>2</button>; }
export function A3() { const [c] = createSignal(0); const h = L.coerce(() => console.log(c())); return <button {...h}>3</button>; }
export function A4() { const [c] = createSignal(0); const h = L.newFn(() => console.log(c())); return <button {...h}>4</button>; }
export function A5() { const [c] = createSignal(0); const h = L.toJson(() => console.log(c())); return <button {...h}>5</button>; }
export function A6() { const [c] = createSignal(0); const h = L.mapValue(() => console.log(c())); return <button {...h}>6</button>; }
export function A7() { const [c] = createSignal(0); const h = L.symbolKey(() => console.log(c())); return <button {...h}>7</button>; }
export function A8() { const [c] = createSignal(0); const h = L.destructDefault(() => console.log(c())); return <button {...h}>8</button>; }
export function A9() { const [c] = createSignal(0); const h = L.threeHop(() => console.log(c())); return <button {...h}>9</button>; }
export function B1() { const [c] = createSignal(0); const h = L.concatKey(() => console.log(c())); return <button {...h}>10</button>; }
export function B2() { const [c] = createSignal(0); const h = L.wrappers(() => console.log(c())); return <button {...h}>11</button>; }
export function B3() { const [c] = createSignal(0); const h = L.sequence(() => console.log(c())); return <button {...h}>12</button>; }
export function B4() { const [c] = createSignal(0); const h = L.instantiation(() => console.log(c())); return <button {...h}>13</button>; }
export function B5() { const [c] = createSignal(0); const h = L.assignProto(() => console.log(c())); return <button {...h}>14</button>; }
export function B6() { const [c] = createSignal(0); const h = L.superGetter(() => console.log(c())); return <button {...h}>15</button>; }
export function B7() { const [c] = createSignal(0); const h = L.superNested(() => console.log(c())); return <button {...h}>16</button>; }
