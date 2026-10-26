// solid-groove src/library/useLibraryBrowser.ts:333 (used by LibraryBrowser.tsx): a factory returns an accessor, a memo and function
// declarations in an object and another component calls them as members; one declaration's text says "this" (here in a comment),
// which the proof reads as a possible use of the object it is called on.
import { createMemo, createSignal } from "solid-js";

export function createBrowser() {
  const [query, setQuery] = createSignal("");
  const hits = createMemo(() => query().length);
  function clear(): void {
    setQuery("");
  }
  function open(): void {
    // this opens the first pack
    setQuery("pack");
  }
  return { query, hits, clear, open };
}

export function Browser() {
  const browser = createBrowser();
  return (
    <button onClick={() => (browser.open(), browser.clear())}>
      {browser.query()} {browser.hits()}
    </button>
  );
}
