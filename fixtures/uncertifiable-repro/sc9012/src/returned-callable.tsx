// jar-hell-web components/Autocomplete.tsx:64 with utils/createDebouncedSignal.ts:
// a project function returns a setter it cast to `Setter<string>`; calling that
// returned callable has no source body to select.
import { createSignal, onCleanup, type Accessor, type Setter } from "solid-js";

function createDebouncedSignal(initial: string, delayMs: number): [Accessor<string>, Accessor<string>, Setter<string>] {
  const [value, setValue] = createSignal(initial);
  const [debounced, setDebounced] = createSignal(initial);
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(timeoutId));
  const set = ((next: string) => {
    const applied = setValue(next);
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => setDebounced(() => applied), delayMs);
    return applied;
  }) as Setter<string>;
  return [value, debounced, set];
}

export function Autocomplete() {
  const [query, , setQuery] = createDebouncedSignal("", 200);
  return <input value={query()} onInput={(event) => setQuery(event.currentTarget.value)} />;
}
