// Violation (style). Corpus: inferay modules/conversation/components/ChatMessageList/UserMessage.tsx:22
// (`cond() && <El/>`, 12 sites), WorkspaceCanvas/index.tsx:766 (ternary with null, 5), SkillEditor/index.tsx:87
// (two-branch ternary, 7): a conditional in JSX that Show expresses.
import { createSignal } from "solid-js";

export function Message() {
  const [images] = createSignal<string[] | null>(null);
  return (
    <div>
      {images() && <b>images</b>}
      {images() ? <i>{images()!.length}</i> : null}
      {images() ? <i>some</i> : <b>none</b>}
    </div>
  );
}
