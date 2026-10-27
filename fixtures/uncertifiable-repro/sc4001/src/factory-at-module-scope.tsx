// Violation. Corpus: kui frontend/packages/feature-messages/src/session.ts:749 (onCleanup in
// createBrowseSession) called from a story's args at module scope, where there is no owner.
import { onCleanup } from "solid-js";

export function createSession(url: string) {
  const source = new EventSource(url);
  onCleanup(() => source.close());
  return source;
}

export const args = { session: createSession("/stream") };
