// Where a value leaves this file, a prop or a context: each holder is walked to its readers, one hop.
import { createContext, createSignal, createStore, useContext } from "solid-js";

const [c] = createSignal(1);
export const aliased = c;
export const holder = { c };

export function useHeld() {
  const [held] = createSignal(1);
  return { held };
}

export function useTaken() {
  const [taken] = createSignal(1);
  return taken;
}

const Ctx = createContext<unknown>();
const StoreCtx = createContext<unknown>();

function Reader() {
  const thrown = useContext(Ctx);
  try { throw thrown; } catch {}
  void JSON.stringify(useContext(StoreCtx));
  return <p>x</p>;
}

function Thrower(p: { v: unknown }) {
  try { throw p.v; } catch {}
  return <p>x</p>;
}

function Spreader(p: { c: unknown }) {
  void Object.values(p);
  return <p>x</p>;
}

export function Provider() {
  const [own] = createSignal(1);
  const [st] = createStore({ a: 1 });
  const bag = { own };
  return (
    <Ctx value={own}>
      <StoreCtx value={st}>
        <Reader />
        <Thrower v={own} />
        <Spreader {...bag} />
      </StoreCtx>
    </Ctx>
  );
}
