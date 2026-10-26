// ai-memory-ui history.ts:33-35: an exported function writes a module-level signal and nothing in
// the project calls it.
import { createSignal } from "solid-js";

const [history, setHistory] = createSignal<string[]>([]);

export { history };

export function recordVisit(path: string): void {
  setHistory((prev) => [path, ...prev.filter((entry) => entry !== path)].slice(0, 20));
}
