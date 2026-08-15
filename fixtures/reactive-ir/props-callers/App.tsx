// Caller-proven component semantics under the 2.0 dialect (probed against
// solid-js@2.0.0-rc.0's devComponent): a prop is signal-backed only when a
// call site passes a reactive expression, module-scope reads run outside
// every strict-read context, and after-await member reads lose their
// dependency exactly like accessor calls. Each section pins one side of
// those facts.
import { createMemo, createSignal, createStore, onSettled } from "solid-js";

// --- module scope -------------------------------------------------------
// The runtime installs strict-read contexts only in component/effect bodies:
// a module-scope read is legal, undiagnosed Solid, so SC1001 stays silent.
const [bootCount] = createSignal(0);
const bootSnapshot = bootCount();

// --- caller-proven props ------------------------------------------------
// Every call site of StaticCard passes static values: its props compile to
// plain properties, so the body read, the parameter destructure, and the
// handler prop are all silent (SC1001, SC1003, SC1007).
function StaticCard(props: { title: string; onSave: () => void }) {
  const title = props.title;
  return <button onClick={props.onSave}>{title}</button>;
}

function StaticDestructure({ title }: { title: string }) {
  return <div title={title} />;
}

function noop() {}

// ReactiveCard's one call site passes a signal read and a store member: the
// props are proven signal-backed, so the body read is an SC1001 violation
// and the handler prop is an SC1007 violation (which owns the expression —
// no SC1001 duplicate on the same span).
function ReactiveCard(props: { title: string; onSave: () => void }) {
  const title = props.title;
  return <button onClick={props.onSave}>{title}</button>;
}

// Destructuring proven-reactive props inside an event handler reads fresh
// values at event time — legal at runtime, so SC1003 stays silent for the
// handler body; the setup-time body destructure above it stays a violation.
function HandlerDestructure(props: { title: string }) {
  const { title } = props;
  const submit = () => {
    const { title: fresh } = props;
    return fresh;
  };
  return <button onClick={submit}>{title}</button>;
}

// A static-only condition cannot re-select the branch: the prop is a plain
// property, so SC1004 (and SC1001) stay silent here.
function StaticBranch(props: { admin: boolean }) {
  if (props.admin) {
    return <div title="admin" />;
  }
  return <div title="user" />;
}

export function App() {
  const [label] = createSignal("live");
  const [handlers] = createStore({ save: noop });
  return (
    <div title={bootSnapshot}>
      <StaticCard title="Hello" onSave={noop} />
      <StaticDestructure title="Hello" />
      <ReactiveCard title={label()} onSave={handlers.save} />
      <HandlerDestructure title={label()} />
      <StaticBranch admin={false} />
    </div>
  );
}

// --- structural branches (SC1004) ---------------------------------------
// A ternary nested inside a JSX attribute of a returned branch is a tracked
// binding, not a structural branch: exactly one SC1004, on the returned
// conditional's own test.
function NestedAttrTernary() {
  const [cond] = createSignal(true);
  const [flag] = createSignal(false);
  return cond() ? <div title={flag() ? "a" : "b"} /> : <div title="c" />;
}

// A logical expression selecting JSX is the same frozen-branch defect as a
// ternary: whichever side wins at setup renders forever.
function LogicalReturn() {
  const [visible] = createSignal(true);
  return visible() && <div title="conditional" />;
}

// A switch discriminant selects the returned tree exactly like an if test.
function SwitchReturn() {
  const [mode] = createSignal(1);
  switch (mode()) {
    case 1:
      return <div title="one" />;
    default:
      return <div title="other" />;
  }
}

// A logical guard over plain data is not a structural branch; the helper's
// reads run in JSX, which tracks. SC1004 stays silent.
function GuardedData() {
  const [visible] = createSignal(true);
  const text = () => visible() && "text";
  return <div title={text()} />;
}

export function Branches() {
  return (
    <div>
      <NestedAttrTernary />
      <LogicalReturn />
      <SwitchReturn />
      <GuardedData />
    </div>
  );
}

// --- derived helpers (SC1006) -------------------------------------------
// Bound and called entirely inside a tracked compute: the reads track where
// the memo runs, so nothing misbehaves.
function DerivedInsideMemo() {
  const [count] = createSignal(0);
  const total = createMemo(() => {
    const doubled = () => count() * 2;
    return doubled();
  });
  return <div title={total()} />;
}

// Bound and called entirely inside onSettled, a deferred leaf callback whose
// reads are legitimately fresh at settle time.
function DerivedInsideSettled() {
  const [count] = createSignal(0);
  onSettled(() => {
    const doubled = () => count() * 2;
    console.log(doubled());
  });
  return <div title="settled" />;
}

export function Derived() {
  return (
    <div>
      <DerivedInsideMemo />
      <DerivedInsideSettled />
    </div>
  );
}

// --- reads after await (SC1002) -----------------------------------------
declare function step(): Promise<void>;

// A store path read after the dominating await registers no dependency: the
// computation never re-runs when the store changes.
function StoreAfterAwait() {
  const [state] = createStore({ value: 1 });
  const total = createMemo(async () => {
    await step();
    return state.value;
  });
  return <div title="store-after-await" />;
}

// A proven-reactive props member read after the await is the same defect.
function PropsAfterAwait(props: { title: string }) {
  const total = createMemo(async () => {
    await step();
    return props.title;
  });
  return <div title="props-after-await" />;
}

// An exported component's callers cannot be enumerated: whether the prop is
// signal-backed is unprovable, so the after-await read is a proof obligation
// (uncertifiable), not a proven violation.
export function ExportedPropsAfterAwait(props: { title: string }) {
  const total = createMemo(async () => {
    await step();
    return props.title;
  });
  return <div title="exported-props-after-await" />;
}

// Reads before the await still register; an await inside a conditional does
// not dominate the read; a nested closure's reads are not the computation's.
function MemberReadsStillTracked(untracked: boolean) {
  const [state] = createStore({ value: 1 });
  const before = createMemo(async () => {
    const seen = state.value;
    await step();
    return seen;
  });
  const conditional = createMemo(async () => {
    if (untracked) {
      await step();
    }
    return state.value;
  });
  const nested = createMemo(async () => {
    await step();
    return () => state.value;
  });
  return <div title="still-tracked" />;
}

export function AfterAwait() {
  const [title] = createSignal("live");
  return (
    <div>
      <StoreAfterAwait />
      <PropsAfterAwait title={title()} />
      <MemberReadsStillTracked untracked={false} />
    </div>
  );
}
