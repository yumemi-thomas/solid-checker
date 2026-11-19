import type { JSX } from "@solidjs/web";
import { createContext, createTrackedEffect, flush } from "solid-js";

// ADR 1123, the review's F2 and F3: rc.21's provider returns `children(() => props.children)`, a memo its reader runs. A child that
// flushes runs where the provider's value is read, so read in a tracked effect it throws FLUSH_IN_TRACKED_QUEUE_CALLBACK; the
// checker keeps the flush's row (SC3001) in each case.

const C = createContext<number>();

// A component that reads what it is given in a tracked effect.
function Reader(props: { value: number; children: JSX.Element }) {
  createTrackedEffect(() => {
    const run = props.children as unknown as () => unknown;
    run();
  });
  return <i />;
}

// F3: the provider element stored, then read in a tracked effect.
function Stored() {
  flush();
  return <i />;
}
export function StoredApp() {
  const element = (
    <C value={1}>
      <Stored />
    </C>
  );
  createTrackedEffect(() => {
    const run = element as unknown as () => unknown;
    run();
  });
  return <i />;
}

// F2: a pattern's name the context's `defaultValue` initializes: the tag is `Reader`, no provider.
const { defaultValue: Picked = Reader } = createContext(Reader);
function Pattern() {
  flush();
  return <i />;
}
export function PatternApp() {
  return (
    <Picked value={1}>
      <Pattern />
    </Picked>
  );
}

// F2: a written `let` holding `Reader`.
let W = createContext<number>() as unknown as typeof Reader;
W = Reader;
function Written() {
  flush();
  return <i />;
}
export function WrittenApp() {
  return (
    <W value={1}>
      <Written />
    </W>
  );
}
