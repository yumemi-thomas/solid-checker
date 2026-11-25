import { createContext, createMemo, refresh, useContext } from "solid-js";
import type { SourceAccessor } from "solid-js";

// ADR 1214 (slice C): a `refresh` target read through a context names the provided source when the context has no default, one
// provider with a closed literal value, and only readers that read static slots. Written in a component body, the write is owned:
// rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE.

type Vals = { m1: SourceAccessor<number> };

// Violation: a member of the `useContext` result.
const MemberCtx = createContext<Vals>();
function MemberReader() {
  const ctx = useContext(MemberCtx);
  refresh(ctx.m1);
  return <p>a</p>;
}
export function MemberParent() {
  const m1 = createMemo(() => 1);
  return (
    <MemberCtx value={{ m1 }}>
      <MemberReader />
    </MemberCtx>
  );
}

// Violation: a destructured slot.
const SlotCtx = createContext<Vals>();
function SlotReader() {
  const { m1 } = useContext(SlotCtx);
  refresh(m1);
  return <p>b</p>;
}
export function SlotParent() {
  const m1 = createMemo(() => 1);
  return (
    <SlotCtx value={{ m1 }}>
      <SlotReader />
    </SlotCtx>
  );
}

// Clean: the same refresh in an event handler.
const HandlerCtx = createContext<Vals>();
function HandlerReader() {
  const ctx = useContext(HandlerCtx);
  return <button onClick={() => refresh(ctx.m1)}>c</button>;
}
export function HandlerParent() {
  const m1 = createMemo(() => 1);
  return (
    <HandlerCtx value={{ m1 }}>
      <HandlerReader />
    </HandlerCtx>
  );
}

// Kept uncertifiable: the context has a default, which a reader under no provider gets.
const fallback = createMemo(() => 0);
const DefaultCtx = createContext<Vals>({ m1: fallback });
function DefaultReader() {
  const ctx = useContext(DefaultCtx);
  refresh(ctx.m1);
  return <p>d</p>;
}
export function DefaultParent() {
  const m1 = createMemo(() => 1);
  return (
    <DefaultCtx value={{ m1 }}>
      <DefaultReader />
    </DefaultCtx>
  );
}

// Kept uncertifiable: two providers, two sources.
const TwoCtx = createContext<Vals>();
function TwoReader() {
  const ctx = useContext(TwoCtx);
  refresh(ctx.m1);
  return <p>e</p>;
}
export function TwoParent() {
  const m1 = createMemo(() => 1);
  const m2 = createMemo(() => 2);
  return (
    <div>
      <TwoCtx value={{ m1 }}>
        <TwoReader />
      </TwoCtx>
      <TwoCtx value={{ m1: m2 }}>
        <TwoReader />
      </TwoCtx>
    </div>
  );
}

// Kept uncertifiable (the member variant): another reader writes the slot.
const WrittenCtx = createContext<Vals>();
const other = createMemo(() => 3);
function WriterReader() {
  const ctx = useContext(WrittenCtx);
  ctx.m1 = other;
  return <p>f</p>;
}
function WrittenReader() {
  const ctx = useContext(WrittenCtx);
  refresh(ctx.m1);
  return <p>g</p>;
}
export function WrittenParent() {
  const m1 = createMemo(() => 1);
  return (
    <WrittenCtx value={{ m1 }}>
      <WriterReader />
      <WrittenReader />
    </WrittenCtx>
  );
}

// Kept uncertifiable (the shadowed variant): the provided name is a `let`, written again.
const LetCtx = createContext<Vals>();
function LetReader() {
  const ctx = useContext(LetCtx);
  refresh(ctx.m1);
  return <p>h</p>;
}
export function LetParent() {
  let m1 = createMemo(() => 1);
  m1 = createMemo(() => 2);
  return (
    <LetCtx value={{ m1 }}>
      <LetReader />
    </LetCtx>
  );
}
