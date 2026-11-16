import { Errored, For, Show, createContext, createTrackedEffect, flush, useContext } from "solid-js";
import { dynamic } from "@solidjs/web";
import * as web from "@solidjs/web";
import { dynamic as projectDynamic } from "./other";

// Default-less and never provided: a read under an owner throws ContextNotFoundError, a plain throw with no diagnostic (ADR 0893).
const Bare = createContext<number>();

// ADR 1016: `const D = dynamic(() => C)` used only as a tag mounts `C` at each `<D />`, so the throw in `C`'s body ends where the tag's does: the
// render around `<D />` reports it unless a boundary above takes it.

// Violations.
function BodyRead() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsBodyRead() {
  const D = dynamic(() => BodyRead);
  return <div><D /></div>;
}

// `flush()` in a tracked effect: a plain throw too (rc.21 "Cannot call flush() from inside onSettled or createTrackedEffect").
function TrackedFlush() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>x</p>;
}
export function MountsTrackedFlush() {
  const D = dynamic(() => TrackedFlush);
  return <div><D></D></div>;
}

// Obligations (uncertifiable): the value is no exact mount, or a boundary above takes the throw.
function UnderBoundary() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsUnderBoundary() {
  const D = dynamic(() => UnderBoundary);
  return <Errored fallback={(error) => <p>{String(error())}</p>}><D /></Errored>;
}

function LetBinding() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsLetBinding() {
  let D = dynamic(() => LetBinding);
  return <div><D /></div>;
}

function UsedOutsideTag() {
  useContext(Bare);
  return <p>x</p>;
}
const kept: unknown[] = [];
export function MountsUsedOutsideTag() {
  const D = dynamic(() => UsedOutsideTag);
  kept.push(D);
  return <div><D /></div>;
}

function BlockBody() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsBlockBody() {
  const D = dynamic(() => {
    return BlockBody;
  });
  return <div><D /></div>;
}

function NamespaceForm() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsNamespaceForm() {
  const D = web.dynamic(() => NamespaceForm);
  return <div><D /></div>;
}

function MemberForm() {
  useContext(Bare);
  return <p>x</p>;
}
const api = { dynamic };
export function MountsMemberForm() {
  const D = api.dynamic(() => MemberForm);
  return <div><D /></div>;
}

function ProjectDynamic() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsProjectDynamic() {
  const D = projectDynamic(() => ProjectDynamic);
  return <div><D /></div>;
}

function Shadowed() {
  useContext(Bare);
  return <p>x</p>;
}
export function MountsShadowed() {
  const dynamic = (source: () => unknown) => () => {
    source();
    return null;
  };
  const D = dynamic(() => Shadowed);
  return <div><D /></div>;
}

function Exported() {
  useContext(Bare);
  return <p>x</p>;
}
export const ExportedMount = dynamic(() => Exported);

// Obligations: `<D />` is written where it never renders, so `C` never mounts (Chrome is silent). Only the tags reach analysis says may run
// count; with none, the throw is unobserved.
function NeverShown() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>x</p>;
}
export function MountsNeverShown() {
  const D = dynamic(() => NeverShown);
  return <Show when={false}><D /></Show>;
}

function FalseAnd() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>x</p>;
}
export function MountsFalseAnd() {
  const D = dynamic(() => FalseAnd);
  return <div>{false && <D />}</div>;
}

function DeadReturn() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>x</p>;
}
export function MountsDeadReturn() {
  const D = dynamic(() => DeadReturn);
  if (false) return <D />;
  return <p>y</p>;
}

function EmptyRows() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>x</p>;
}
export function MountsEmptyRows() {
  const D = dynamic(() => EmptyRows);
  return <For each={[]}>{() => <D />}</For>;
}
