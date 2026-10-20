import { createSignal } from "solid-js";
import * as L from "./lib";

export function Promise_() { const [c] = createSignal(0); const h = L.viaPromise(() => console.log(c())); return <button {...h}>a</button>; }
export function Map_() { const [c] = createSignal(0); const h = L.viaMap(() => console.log(c())); return <button {...h}>b</button>; }
export function Sort_() { const [c] = createSignal(0); const h = L.viaSort(() => console.log(c())); return <button {...h}>c</button>; }
export function Replace_() { const [c] = createSignal(0); const h = L.viaReplace(() => console.log(c())); return <button {...h}>d</button>; }
export function Default_() { const [c] = createSignal(0); const h = L.viaDefault(() => console.log(c())); return <button {...h}>e</button>; }
export function Getter_() { const [c] = createSignal(0); const h = L.viaGetter(() => console.log(c())); return <button {...h}>f</button>; }
export function Property_() { const [c] = createSignal(0); const h = L.viaProperty(() => console.log(c())); h.k(); return <button onClick={h.onClick}>g</button>; }
export function CallApply_() { const [c] = createSignal(0); const h = L.viaCallApply(() => console.log(c())); return <button {...h}>h</button>; }
export function Optional_() { const [c] = createSignal(0); const h = L.viaOptional(() => console.log(c())); return <button {...h}>i</button>; }
export function Container_() { const [c] = createSignal(0); const h = L.viaContainer(() => console.log(c())); L.drain(); return <button {...h}>j</button>; }
export function ProjectFn_() { const [c] = createSignal(0); const h = L.viaProjectFn(() => console.log(c())); return <button {...h}>k</button>; }
export function ReturnAlias_() { const [c] = createSignal(0); const { g } = L.viaReturnAlias(() => console.log(c())); g(); return <button>l</button>; }
export function ReturnAlias2_() { const [c] = createSignal(0); const h = L.viaReturnAlias(() => console.log(c())); const g = h.g; g(); return <button onClick={h.onClick}>m</button>; }
