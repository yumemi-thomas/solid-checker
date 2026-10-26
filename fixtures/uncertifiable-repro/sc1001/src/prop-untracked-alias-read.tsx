// inferay AgentChatView/AgentChatMessages.tsx:55-61 (`virtualizerControlsRef` reads `props.state`): a child snapshots a callback prop with
// `untrack(() => props.controlsRef)`, so the callback's reads of a component prop leave the walk through `props.controlsRef`.
import { untrack } from "solid-js";

type Viewport = { handle: number };

function MessageList(props: { controlsRef: (handle: number) => void }) {
  const controlsRef = untrack(() => props.controlsRef);
  return <div ref={() => controlsRef(1)} />;
}

export function Messages(props: { viewport: Viewport }) {
  return <MessageList controlsRef={(handle) => { props.viewport.handle = handle; }} />;
}
