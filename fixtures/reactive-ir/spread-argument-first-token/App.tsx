import { createSignal } from "solid-js";

type Registry = { readonly size: number };
type P = { readonly id: string; readonly queries?: Registry | undefined };

function sizeOf(options: { registry?: Registry }): number {
  return options.registry?.size ?? 0;
}

// No finding: the spread argument is a conditional that merely STARTS with
// `props`. It spreads a fresh object, not the props object. No caller passes
// `queries`, so the two member reads are static.
function ConditionalSpread(props: P) {
  const size = sizeOf({
    ...(props.queries === undefined ? {} : { registry: props.queries }),
  });
  return <div title={props.id}>{size}</div>;
}

// No finding: the same, with no member read in a branch.
function ConditionalNoRead(props: P) {
  const size = sizeOf({
    ...(props.queries === undefined ? {} : {}),
  });
  return <div title={props.id}>{size}</div>;
}

// No finding: a logical expression that starts with `props`.
function LogicalSpread(props: P) {
  const size = sizeOf({
    ...(props.queries && {}),
  });
  return <div title={props.id}>{size}</div>;
}

// Violations at the two `props.queries` reads (the caller passes a reactive
// `queries`), judged as ordinary member reads. No "props spread" read: the
// spread is not of the props object.
function ReactiveCaller(props: P) {
  const size = sizeOf({
    ...(props.queries === undefined ? {} : { registry: props.queries }),
  });
  return <div title={props.id}>{size}</div>;
}

// A real spread of the props object, parenthesized: not changed by the fix
// (uncertifiable, not a violation).
function RealSpread(props: P) {
  const size = sizeOf({
    ...(props),
  });
  return <div title={props.id}>{size}</div>;
}

export function Route() {
  const [id] = createSignal("a");
  const [queries] = createSignal<Registry | undefined>(undefined);
  return (
    <div>
      <ConditionalSpread id={id()} />
      <ConditionalNoRead id={id()} />
      <LogicalSpread id={id()} />
      <ReactiveCaller id={id()} queries={queries()} />
      <RealSpread id={id()} />
    </div>
  );
}
