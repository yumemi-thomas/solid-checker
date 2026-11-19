import { createContext, createTrackedEffect, flush } from "solid-js";
import { makeReader, track } from "dep-track";

// ADR 1123, the review's F2 and F3, through a package the build does not read (dep-track names and imports no Solid package): its
// functions run what they are given in the `createTrackedEffect` the caller passes. A flushing child expression then runs inside the
// tracked effect: Chrome reports FLUSH_IN_TRACKED_QUEUE_CALLBACK, covered by the flush's SC3001 row (the reader's own tracked effect keeps it,
// before round 1 too).

const C = createContext<number>();

// F3: the provider element stored, then handed to the package, which reads it in a tracked effect.
export function StoredApp() {
  const element = (
    <C value={1}>
      {(() => {
        flush();
        return <i />;
      })()}
    </C>
  );
  track(createTrackedEffect, () => element);
  return <i />;
}

// F2: a pattern's name the context's `defaultValue` initializes: the tag is the package's reader, no provider.
const PkgReader = makeReader(createTrackedEffect);
const { defaultValue: Picked = PkgReader } = createContext(PkgReader);
export function PatternApp() {
  return (
    <Picked value={1}>
      {(() => {
        flush();
        return <i />;
      })()}
    </Picked>
  );
}

// F2: a written `let` that holds the package's reader when the tag runs.
let W = createContext<number>() as unknown as typeof PkgReader;
W = PkgReader;
export function WrittenApp() {
  return (
    <W value={1}>
      {(() => {
        flush();
        return <i />;
      })()}
    </W>
  );
}
