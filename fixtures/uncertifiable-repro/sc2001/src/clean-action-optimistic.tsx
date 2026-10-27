// Clean. Mirrors hoxxes-briefing apps/web/src/pages/briefing/ui/board/RefreshPanel.tsx:63-82: an
// action writes an optimistic flag and a counter around a yield, and an effect with a previous
// value turns the counter into a flash.
import { action, createEffect, createOptimistic, createSignal } from "solid-js";

function Refresh(props: { onRefresh: () => void; failed: boolean }) {
  const [isLoading, setIsLoading] = createOptimistic(false);
  const [flash, setFlash] = createSignal<string | null>(null);
  const [settled, setSettled] = createSignal(0);
  const handleRefresh = action(function* () {
    setFlash(null);
    setIsLoading(true);
    props.onRefresh();
    yield;
    setSettled((count) => count + 1);
  });
  createEffect(
    () => ({ attempt: settled(), failed: props.failed }),
    (value, previous) => {
      if (previous != null && value.attempt !== previous.attempt) {
        setFlash(value.failed ? "danger" : "success");
      }
    },
  );
  return (
    <button
      type="button"
      aria-busy={isLoading() ? "true" : "false"}
      data-flash={flash() ?? undefined}
      onClick={handleRefresh}
    >
      refresh
    </button>
  );
}

export function App() {
  return <Refresh onRefresh={() => console.log("refresh")} failed={false} />;
}
