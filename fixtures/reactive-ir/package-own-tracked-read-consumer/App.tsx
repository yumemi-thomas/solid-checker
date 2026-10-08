import { watchStatus, peekStatus } from "reactive-package";
import { peekFromModule } from "./peek";

// The contract states the read `tracked`, under an owner the export creates:
// the export's own computation observes it, so nothing is read untracked here.
export function Watched() {
  watchStatus();
  return <div />;
}

// The contract states the read untracked, on the call's stack. Solid warns at
// every use, so it is the package's own read: uncertifiable, not a violation.
export function Peeked() {
  peekStatus();
  return <div />;
}

// The same untracked own read, reached through project wrappers. The read is
// still the package's own state; the wrapper does not make it the caller's.
function peekThroughWrapper() {
  peekStatus();
}

export function PeekedThroughWrapper() {
  peekThroughWrapper();
  return <div />;
}

export function PeekedThroughModule() {
  peekFromModule();
  return <div />;
}
