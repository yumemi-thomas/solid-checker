// Clean. Mirrors inferay src/modules/conversation/components/ChatPaneBoundary/index.tsx:5-12: Errored
// whose fallback receives the error accessor and a reset function and passes them down.
import { Errored } from "solid-js";

function Recovery(props: { error: unknown; reset: () => void }) {
  return <button type="button" onClick={() => props.reset()}>{String(props.error)}</button>;
}

function Broken(): never {
  throw new Error("boom");
}

export function Pane() {
  return (
    <Errored fallback={(error, reset) => <Recovery error={error()} reset={reset} />}>
      <Broken />
    </Errored>
  );
}
