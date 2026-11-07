// Clean. Mirrors ai-memory-ui src/components/page-reader.tsx:28-39 (and 4 more copy buttons): a
// handler writes a flag after a promise settles and resets it on a timer cleared by onCleanup.
import { createSignal, onCleanup } from "solid-js";

const copyText = (value: string) => Promise.resolve(value);

function CopyPath(props: { path: string }) {
  const [copied, setCopied] = createSignal(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(copiedTimer));
  const copyPath = () => {
    void copyText(props.path).then(() => {
      setCopied(true);
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => setCopied(false), 1500);
    });
  };
  return <button type="button" onClick={copyPath}>{copied() ? "copied" : props.path}</button>;
}

export function App() {
  return <CopyPath path="a/b" />;
}
