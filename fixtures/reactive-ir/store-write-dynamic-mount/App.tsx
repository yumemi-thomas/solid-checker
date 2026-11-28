import { dynamic } from "@solidjs/web";
import * as web from "@solidjs/web";
import { useClosing, useInWindow, useMounted, useNamespace, useOutsideTag, useShadowed, useThroughSetter } from "./hooks";

// ADR 1234: a component mounted only through `const D = dynamic(() => C)` and `<D />` (ADR 1016's reading) starts where `D`'s tags are:
// `dynamic`'s component renders `C` in a memo of its own. A store write past its root there is dropped outside the setter
// (STORE_WRITE_OUTSIDE_SETTER) unless the call or a tag is in a function a setter may run.

// Violations.
function Mounted() {
  const { s1 } = useMounted();
  s1.a.v = 7;
  return <i />;
}
export function MountsMounted() {
  const D = dynamic(() => Mounted);
  return <div><D /></div>;
}

function Closing() {
  const { s1 } = useClosing();
  s1.items.push(1);
  return <i />;
}
export function MountsClosing() {
  const D = dynamic(() => Closing);
  return <div><D></D></div>;
}

// Clean: the write is the setter's own.
function ThroughSetter() {
  const { setS1 } = useThroughSetter();
  setS1((draft) => {
    draft.a.v = 7;
  });
  return <i />;
}
export function MountsThroughSetter() {
  const D = dynamic(() => ThroughSetter);
  return <div><D /></div>;
}

// Uncertifiable.
declare function later(render: () => unknown): void;

// The tag is written in a function handed to a call that may be a setter.
function InWindow() {
  const { s1 } = useInWindow();
  s1.a.v = 7;
  return <i />;
}
export function MountsInWindow() {
  const D = dynamic(() => InWindow);
  later(() => <D />);
  return <div />;
}

// `D` is used outside a tag: no reading.
function OutsideTag() {
  const { s1 } = useOutsideTag();
  s1.a.v = 7;
  return <i />;
}
const mounted: unknown[] = [];
export function MountsOutsideTag() {
  const D = dynamic(() => OutsideTag);
  mounted.push(D);
  return <div><D /></div>;
}

// A local `dynamic` shadows the import.
function Shadowed() {
  const { s1 } = useShadowed();
  s1.a.v = 7;
  return <i />;
}
export function MountsShadowed() {
  const dynamic = (source: () => () => JSX.Element) => () => source()();
  const D = dynamic(() => Shadowed);
  return <div><D /></div>;
}

// `web.dynamic`: the bare-name check refuses a namespace callee.
function Namespace() {
  const { s1 } = useNamespace();
  s1.a.v = 7;
  return <i />;
}
export function MountsNamespace() {
  const D = web.dynamic(() => Namespace);
  return <div><D /></div>;
}
