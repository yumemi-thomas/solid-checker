// skyjtx-website Carousel.tsx:54,128,154; NavigationBar.tsx:30,68; Tabs.tsx:174: signal writes and reads in event
// handlers of a project whose test file dispatches an event on a receiver it created (ADR 0356): a write in the `goTo`
// helper, a write in a helper a child delivers through an intrinsic handler, a write in a handler, a read in a handler.
import { createMemo, createSignal } from "solid-js";

function Reset(props: { onReset: () => void }) {
  return <button onClick={() => props.onReset()}>Reset</button>;
}

export function Carousel(props: { count: number }) {
  const [currentPage, setCurrentPage] = createSignal(0);
  const totalPages = createMemo(() => Math.max(1, props.count));
  const goTo = (page: number) => {
    setCurrentPage(Math.max(0, Math.min(page, totalPages() - 1)));
  };
  const reset = () => {
    setCurrentPage(0);
  };
  return (
    <div>
      <button disabled={currentPage() === 0} onClick={() => goTo(currentPage() - 1)}>
        Previous
      </button>
      <button onClick={() => setCurrentPage(totalPages() - 1)}>Last</button>
      <Reset onReset={reset} />
    </div>
  );
}
