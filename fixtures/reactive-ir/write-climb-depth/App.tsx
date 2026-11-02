import { createRoot, createSignal, For } from "solid-js";

// d11: the write directly in the root body (violation).
export function RootZero() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => createRoot(() => { setW1(7); })} />;
}

// d9: eight IIFE layers under the root body (violation).
export function RootEight() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => createRoot(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); })} />;
}

// d2c: fifteen layers; the disposal climb's bound makes it an obligation.
export function RootFifteen() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => createRoot(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })} />;
}

// d2b: sixteen layers, the owner climb's bound: an obligation, never silence (ADR 0532).
export function RootSixteen() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => createRoot(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })} />;
}

// d2: seventeen layers, past every bound: an obligation (ADR 0532).
export function RootSeventeen() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => createRoot(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })} />;
}

// Legal twin: eight layers directly in the handler, no owner (clean).
export function HandlerEight() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); }} />;
}

// Legal twin past the bound: no owner, but unread past 16 layers (obligation, a recorded trade).
export function HandlerSeventeen() {
  const [w1, setW1] = createSignal(0);
  void w1;
  return <button onClick={() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { setW1(7); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); }} />;
}

// Wrapper variant: each helper calls its callback synchronously.
export function RootHelperSixteen() {
  const [, setW1] = createSignal(0);
  const h = (cb: () => void) => cb();
  return <button onClick={() => createRoot(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { h(() => { setW1(7); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); }); })} />;
}

// Read-role consumer of an exhausted partial climb: obligation, never deferred.
export function FlowReadSixteen() {
  const [r1] = createSignal(0);
  return <For each={[1]}>{() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { r1(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); return <button />; }}</For>;
}
