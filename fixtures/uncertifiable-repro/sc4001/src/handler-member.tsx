// every-deck-of-cards ExplorerFeed.tsx:30: a callback prop is the handler
// itself (`onClick={props.onRetry}`), not a literal the analysis reads.
import { createSignal } from "solid-js";

function ExplorerFeed(props: { failed: boolean; onRetry: () => void }) {
  return (
    <div role="alert">
      <p>{props.failed ? "These decks could not be loaded." : "Loaded."}</p>
      <button type="button" onClick={props.onRetry}>
        Try again
      </button>
    </div>
  );
}

export function Explorer() {
  const [tries, setTries] = createSignal(0);
  return <ExplorerFeed failed={tries() < 3} onRetry={() => setTries(tries() + 1)} />;
}
