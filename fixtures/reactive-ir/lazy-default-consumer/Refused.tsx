import { lazy } from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};
const flag = Math.random() > 0.5;

// Each loader below misses one premise of the lazy-default render witness,
// so its component's `start()` keeps the object-retention refusal.

// `{ export: "Other" }` selects another member: `NamedCube` is never the
// component this wrapper renders.
const NamedLoop = lazy(
  async () => {
    function NamedCube() {
      const [running, start] = createLoop(frame);
      start();
      return <div>{String(running())}</div>;
    }
    function Other() {
      return <div />;
    }
    return { default: NamedCube, Other };
  },
  { export: "Other" }
);

// Two returns: the loader selects between module objects.
const EitherLoop = lazy(async () => {
  function EitherCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  function Fallback() {
    return <div />;
  }
  if (flag) return { default: EitherCube };
  return { default: Fallback };
});

// A mutable binding, written before the return.
const MutableLoop = lazy(async () => {
  let MutableCube = () => {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  };
  if (flag) MutableCube = () => <div />;
  return { default: MutableCube };
});

// An aliased module object, whose `default` is replaced before it returns.
const AliasLoop = lazy(async () => {
  function AliasCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  const mod = { default: AliasCube };
  if (flag) mod.default = () => <div />;
  return mod;
});

// A spread module object: the spread may supply members the literal hides.
const base = {};
const SpreadLoop = lazy(async () => {
  function SpreadCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { ...base, default: SpreadCube };
});

// Another retaining object holds the same function.
export const registry: Record<string, () => unknown> = {};
const RegistryLoop = lazy(async () => {
  function RegistryCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  registry.cube = RegistryCube;
  return { default: RegistryCube };
});

// The loader is a reference, not the call's own literal.
async function loadReferenced() {
  function ReferencedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: ReferencedCube };
}
const ReferencedLoop = lazy(loadReferenced);

// `.preload()` hands the module promise to a caller that replaces `default`.
const SwappedLoop = lazy(async () => {
  function SwappedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: SwappedCube };
});
void SwappedLoop.preload().then((mod) => {
  mod.default = () => <div />;
});

// Only preloaded, never rendered.
const PreloadedLoop = lazy(async () => {
  function PreloadedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: PreloadedCube };
});

// Never used at all.
const UnusedLoop = lazy(async () => {
  function UnusedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: UnusedCube };
});
void UnusedLoop;

// Exported: another module may preload it.
export const ExportedLoop = lazy(async () => {
  function ExportedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: ExportedCube };
});

// A function that merely returns a component is not one.
const FactoryLoop = lazy(async () => {
  function makeCube() {
    const [running, start] = createLoop(frame);
    start();
    return () => <div>{String(running())}</div>;
  }
  return { default: makeCube() };
});

export function RefusedPage() {
  void PreloadedLoop.preload();
  return (
    <main>
      <NamedLoop />
      <EitherLoop />
      <MutableLoop />
      <AliasLoop />
      <SpreadLoop />
      <RegistryLoop />
      <ReferencedLoop />
      <SwappedLoop />
      <ExportedLoop />
      <FactoryLoop />
    </main>
  );
}
