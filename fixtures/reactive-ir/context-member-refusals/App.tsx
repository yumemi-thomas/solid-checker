import { createContext, createSignal, useContext } from "solid-js";
const Two = createContext<{ two: () => number }>();
export function FirstProvider() { const [two] = createSignal(1); return <Two value={{two}}><p>first</p></Two>; }
export function SecondProvider() { const [two] = createSignal(2); return <Two value={{two}}><p>second</p></Two>; }
export function TwoProviders() { const value = useContext(Two); const n = value.two(); return <p>{n}</p>; }
const Guarded = createContext<{ guarded: () => number }>();
export function GuardedProvider() { const [guarded] = createSignal(1); return <Guarded value={{guarded}}><p>guarded</p></Guarded>; }
function useGuarded() { const value = useContext(Guarded); if (!value) throw new Error("missing"); return value; }
export function GuardedConsumer() { const value = useGuarded(); const n = value.guarded(); return <p>{n}</p>; }
const Aliased = createContext<{ aliased: () => number }>();
export function AliasProvider() { const [aliased] = createSignal(1); return <Aliased value={{aliased}}><p>alias</p></Aliased>; }
function useAlias() { const value = useContext(Aliased); const alias = value; return alias; }
export function AliasConsumer() { const value = useAlias(); const n = value.aliased(); return <p>{n}</p>; }
const Plain = createContext<{ plain: () => number }>();
export function PlainProvider() { return <Plain value={{plain: () => 0}}><p>plain</p></Plain>; }
export function PlainConsumer() { const value = useContext(Plain); const n = value.plain(); return <p>{n}</p>; }
const Mutated = createContext<{ bad: () => number }>();
export function MutationProvider() { const [bad] = createSignal(1); return <Mutated value={{bad}}><p>mutation</p></Mutated>; }
function useMutated() { const value = useContext(Mutated); return value; }
function replace(read: () => {bad: () => number}) { read().bad = () => 0; }
export function MutationConsumer() { replace(useMutated); const value = useContext(Mutated); const n = value.bad(); return <p>{n}</p>; }
const Escaped = createContext<{ escaped: () => number }>();
export function EscapeProvider() { const [escaped] = createSignal(1); return <Escaped value={{escaped}}><p>escape</p></Escaped>; }
declare function retain(value: {escaped: () => number}): void;
function useEscaped() { const value = useContext(Escaped); retain(value); return value; }
export function EscapeConsumer() { const value = useEscaped(); const n = value.escaped(); return <p>{n}</p>; }
