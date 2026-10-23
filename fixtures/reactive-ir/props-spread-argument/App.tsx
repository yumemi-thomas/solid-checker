import { createSignal } from "solid-js";

type Registry = { readonly name: string };
type P = { id: string; queries?: Registry };

// Reads nothing of its options, so the spread is the only read of interest.
function useQuery(_options: { registry?: Registry; name?: string; id?: string }) {
  return () => "listing";
}

const enabled = Math.random() > 0.5;

// ---- Not a props spread: the argument is not the props object.

// Clean: the kui shape (KsqlRoute.tsx:164). The argument is a conditional
// whose results are `{}` and a fresh literal; spreading it runs none of the
// props' getters. Its only prop read is `props.queries`, which no caller of
// this component passes reactively.
function ConditionalOptions(props: P) {
  const listing = useQuery({
    ...(props.queries === undefined ? {} : { registry: props.queries }),
  });
  return <p>{listing()}</p>;
}

// Clean: a logical whose result is the literal or a falsy test value.
function LogicalOptions(props: P) {
  const listing = useQuery({ ...(props.queries && { registry: props.queries }) });
  return <p>{listing()}</p>;
}

// Clean: an object literal argument.
function LiteralOptions(props: P) {
  const listing = useQuery({ ...{ registry: props.queries } });
  return <p>{listing()}</p>;
}

// Clean: a member argument reads one prop, `props.queries`, not the props.
function MemberOptions(props: P) {
  const listing = useQuery({ ...props.queries });
  return <p>{listing()}</p>;
}

// Violations at the two `props.queries` reads, none at the spread: the same
// shape as ConditionalOptions, but a caller passes `queries` reactively, so
// each member read is a setup read of a signal-backed prop.
function ReactiveQueries(props: P) {
  const listing = useQuery({
    ...(props.queries === undefined ? {} : { registry: props.queries }),
  });
  return <p>{listing()}</p>;
}

// ---- A props spread.

// Uncertifiable: the props object itself, spread in setup, runs every getter,
// but a spread into an object literal is not one of ADR 0337's inert uses, so
// the props root is not proven current.
function PropsOptions(props: P) {
  const listing = useQuery({ ...props });
  return <p>{listing()}</p>;
}

// Uncertifiable: the props object is one branch's result, so the spread runs
// its getters only when that branch is taken; a branch value is an obligation.
function ConditionalProps(props: P) {
  const listing = useQuery({ ...(enabled ? props : {}) });
  return <p>{listing()}</p>;
}

// Uncertifiable: as above, with the test itself a prop read (the `props.queries`
// read is static: no caller passes it reactively).
function ConditionalOnProp(props: P) {
  const listing = useQuery({ ...(props.queries ? props : {}) });
  return <p>{listing()}</p>;
}

// Uncertifiable: the right operand of `&&` is the result when the test holds.
function LogicalProps(props: P) {
  const listing = useQuery({ ...(enabled && props) });
  return <p>{listing()}</p>;
}

export function App() {
  const [id, setId] = createSignal("c1");
  const [registry] = createSignal<Registry>({ name: "live" });
  const queries: Registry = { name: "test" };
  return (
    <>
      <button onClick={() => setId("c9")} />
      <ConditionalOptions id={id()} />
      <ConditionalOptions id="c2" queries={queries} />
      <LogicalOptions id={id()} queries={queries} />
      <LiteralOptions id={id()} queries={queries} />
      <MemberOptions id={id()} queries={queries} />
      <ReactiveQueries id="c3" queries={registry()} />
      <PropsOptions id={id()} />
      <ConditionalProps id={id()} />
      <ConditionalOnProp id={id()} queries={queries} />
      <LogicalProps id={id()} />
    </>
  );
}
