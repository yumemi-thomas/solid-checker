import { createContext, createSignal, useContext } from "solid-js";
const Two = createContext<{ two: () => number }>();
export function FirstProvider() { const [two] = createSignal(1); return <Two value={{two}}><p>first</p></Two>; }
export function SecondProvider() { const [two] = createSignal(2); return <Two value={{two}}><p>second</p></Two>; }
export function TwoProviders() { const value = useContext(Two); const n = value.two(); return <p>{n}</p>; }
const Guarded = createContext<{ guarded: () => number }>();
export function GuardedProvider() { const [guarded] = createSignal(1); return <Guarded value={{guarded}}><p>guarded</p></Guarded>; }
function useGuarded() { const value = useContext(Guarded); if (!value) throw new Error("missing"); const alias = value; void alias; return value; }
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

const Defaulted = createContext<{ defaultRead: () => number }>();
export function DefaultProvider() { const [defaultRead] = createSignal(1); return <Defaulted value={{defaultRead}}><p>provider</p></Defaulted>; }
export function DefaultConsumer() { const { defaultRead = () => 0 } = useContext(Defaulted); const n = defaultRead(); return <p>{n}</p>; }
const Rest = createContext<{ restRead: () => number }>();
export function RestProvider() { const [restRead] = createSignal(1); return <Rest value={{restRead}}><p>provider</p></Rest>; }
export function RestConsumer() { const { restRead, ...rest } = useContext(Rest); void rest; const n = restRead(); return <p>{n}</p>; }
const EscapedSlot = createContext<{ slotRead: () => number }>();
export function SlotProvider() { const [slotRead] = createSignal(1); return <EscapedSlot value={{slotRead}}><p>provider</p></EscapedSlot>; }
declare function retainSlot(read: () => number): void;
export function SlotConsumer() { const { slotRead } = useContext(EscapedSlot); retainSlot(slotRead); const n = slotRead(); return <p>{n}</p>; }
const CoalesceEscape = createContext<{ escapedNullish: () => number }>();
export function CoalesceEscapeProvider() { const [escapedNullish] = createSignal(1); return <CoalesceEscape value={{escapedNullish}}><p>provider</p></CoalesceEscape>; }
function useCoalesceEscape() { const value = useContext(CoalesceEscape); const alias = value ?? { escapedNullish: () => 0 }; void alias; return value; }
export function CoalesceEscapeConsumer() { const value = useCoalesceEscape(); const n = value.escapedNullish(); return <p>{n}</p>; }

const Quoted = createContext<{ quotedRead: () => number }>();
export function QuotedProvider() { const [quotedRead] = createSignal(1); return <Quoted value={{quotedRead}}><p>provider</p></Quoted>; }
export function QuotedConsumer() { const { "quotedRead": read } = useContext(Quoted); const n = read(); return <p>{n}</p>; }
const EscapedKey = createContext<{ escapedRead: () => number }>();
export function EscapedKeyProvider() { const [escapedRead] = createSignal(1); return <EscapedKey value={{escapedRead}}><p>provider</p></EscapedKey>; }
export function EscapedKeyConsumer() { const { escap\u0065dRead: read } = useContext(EscapedKey); const n = read(); return <p>{n}</p>; }
