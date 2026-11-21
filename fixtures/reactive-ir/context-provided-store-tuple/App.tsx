import { createContext, createSignal, createStore, createTrackedEffect, useContext, type Accessor, type Store } from "solid-js";

// ADR 1147, arm 3 (`context-provided-store-tuple`): a context whose one value is a fresh `[store, { actions }]` literal. Each reader's slot-0 name is
// the store itself, and each action's local is only called, so a store read in a reader has its store rows and a write in an action is placed by the
// readers' calls. Any other form keeps its rows (SC9012 at the value and the reads, SC2001 at the write).

type Cart = { items: string[] };
declare function keep(value: unknown): void;

// Positive: textbook id-context's shape.
const CartContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function CartProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <CartContext value={[cart, { add }] as const}>{props.children}</CartContext>;
}
function AddButton() {
  const [, { add }] = useContext(CartContext);
  return <button onClick={() => add("apple")}>add</button>;
}
function Count() {
  const [cart] = useContext(CartContext);
  return <span>{cart.items.length}</span>;
}
export function Shop() {
  return (
    <CartProvider>
      <AddButton />
      <Count />
    </CartProvider>
  );
}

// Negative: a reader holds the tuple whole: a second holder of the action object.
const WholeContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function WholeProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <WholeContext value={[cart, { add }] as const}>{props.children}</WholeContext>;
}
function WholeReader() {
  const tuple = useContext(WholeContext);
  return <button onClick={() => tuple[1].add("pear")}>{tuple[0].items.length}</button>;
}
export function WholeApp() {
  return (
    <WholeProvider>
      <WholeReader />
    </WholeProvider>
  );
}

// Negative: an action's local handed on (an alias), not only called.
const AliasContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function AliasProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <AliasContext value={[cart, { add }] as const}>{props.children}</AliasContext>;
}
function AliasReader() {
  const [cart, { add }] = useContext(AliasContext);
  const run = add;
  return <button onClick={() => run("plum")}>{cart.items.length}</button>;
}
export function AliasApp() {
  return (
    <AliasProvider>
      <AliasReader />
    </AliasProvider>
  );
}

// Negative: slot 1 bound whole: the action object has a holder.
const HeldContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function HeldProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <HeldContext value={[cart, { add }] as const}>{props.children}</HeldContext>;
}
function HeldReader() {
  const [cart, actions] = useContext(HeldContext);
  return <button onClick={() => actions.add("fig")}>{cart.items.length}</button>;
}
export function HeldApp() {
  return (
    <HeldProvider>
      <HeldReader />
    </HeldProvider>
  );
}

// Negative: the value is a `const` with a second reference (handed on).
const KeptContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function KeptProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  const tuple = [cart, { add }] as const;
  keep(tuple);
  return <KeptContext value={tuple}>{props.children}</KeptContext>;
}
function KeptReader() {
  const [cart, { add }] = useContext(KeptContext);
  return <button onClick={() => add("kiwi")}>{cart.items.length}</button>;
}
export function KeptApp() {
  return (
    <KeptProvider>
      <KeptReader />
    </KeptProvider>
  );
}

// Negative: a second provider of the same context.
const TwiceContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function TwiceProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <TwiceContext value={[cart, { add }] as const}>{props.children}</TwiceContext>;
}
function TwiceReader() {
  const [cart, { add }] = useContext(TwiceContext);
  return <button onClick={() => add("lime")}>{cart.items.length}</button>;
}
export function TwiceApp() {
  const [other] = createStore<Cart>({ items: [] });
  return (
    <TwiceProvider>
      <TwiceContext value={[other, { add: (item: string) => keep(item) }] as const}>
        <TwiceReader />
      </TwiceContext>
    </TwiceProvider>
  );
}

// Negative: slot 0 is a signal's accessor, no store.
const SignalContext = createContext<readonly [Accessor<number>, { bump: () => void }]>();
function SignalProvider(props: { children?: JSX.Element }) {
  const [count, setCount] = createSignal(0);
  const bump = () => setCount((value) => value + 1);
  return <SignalContext value={[count, { bump }] as const}>{props.children}</SignalContext>;
}
function SignalReader() {
  const [count, { bump }] = useContext(SignalContext);
  return <button onClick={() => bump()}>{count()}</button>;
}
export function SignalApp() {
  return (
    <SignalProvider>
      <SignalReader />
    </SignalProvider>
  );
}

// Negative (review a): an action's local used as a value, not called: handed to a scheduler.
const QueuedContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function QueuedProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <QueuedContext value={[cart, { add }] as const}>{props.children}</QueuedContext>;
}
function QueuedReader() {
  const [cart, { add }] = useContext(QueuedContext);
  const f = add;
  queueMicrotask(() => f("date"));
  return <span>{cart.items.length}</span>;
}
export function QueuedApp() {
  return (
    <QueuedProvider>
      <QueuedReader />
    </QueuedProvider>
  );
}

// Positive, with its other rows standing (review b): the store's setter also handed to code nothing here reads. The setter's own census keeps that
// hand-off's rows; the reader's store name is the same store, so whatever the root's census says of a write there it says of the reader's reads too.
const LeakedContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function LeakedProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  keep(setCart);
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <LeakedContext value={[cart, { add }] as const}>{props.children}</LeakedContext>;
}
function LeakedReader() {
  const [cart, { add }] = useContext(LeakedContext);
  return <button onClick={() => add("yuzu")}>{cart.items.length}</button>;
}
export function LeakedApp() {
  return (
    <LeakedProvider>
      <LeakedReader />
    </LeakedProvider>
  );
}

// Negative (the review of 1147, F1): an action's local called in a callback handed to other code, which runs it where it says (here a project helper that
// calls it in `createTrackedEffect`; the review's probe, a package read as source that does the same, gave SC2001 a violation Chrome never reports). The arm
// refuses the plan: the write keeps its row.
function hand(callback: () => void) {
  createTrackedEffect(() => callback());
}
const HandedContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function HandedProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <HandedContext value={[cart, { add }] as const}>{props.children}</HandedContext>;
}
function HandedReader() {
  const [cart, { add }] = useContext(HandedContext);
  hand(() => add("x"));
  return <span>{cart.items.length}</span>;
}
export function HandedApp() {
  return (
    <HandedProvider>
      <HandedReader />
    </HandedProvider>
  );
}

// Negative (F1): the same through a callee nothing here reads.
declare function handOut(callback: () => void): void;
const OutContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function OutProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <OutContext value={[cart, { add }] as const}>{props.children}</OutContext>;
}
function OutReader() {
  const [cart, { add }] = useContext(OutContext);
  handOut(() => add("y"));
  return <span>{cart.items.length}</span>;
}
export function OutApp() {
  return (
    <OutProvider>
      <OutReader />
    </OutProvider>
  );
}
