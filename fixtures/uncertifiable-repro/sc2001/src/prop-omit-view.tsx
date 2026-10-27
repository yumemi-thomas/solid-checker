// hoxxes-briefing ShareButton.tsx:38 (IconButton at shared/ui/icon-button/index.tsx:84): a function is the child's `onClick`, and the
// child reads its props through `omit` and spreads the rest onto a native button; the walk does not enter a view built by `omit`.
import { createSignal, omit } from "solid-js";

function IconButton(props: { label: string; onClick?: (event: MouseEvent) => void }) {
  const rest = omit(props, "label");
  return <button aria-label={props.label} {...rest} />;
}

export function Share() {
  const [flash, setFlash] = createSignal(false);
  function handleShare(): void {
    setFlash(true);
  }
  return <IconButton label={flash() ? "Copied" : "Share"} onClick={handleShare} />;
}
