// inferay useChatDraft.tsx:21 (consumer useAgentChatState.tsx:66,144): a function is taken out of a hook's result and handed to
// `merge`, which builds a view of several objects the walk does not enter.
import { createSignal, merge } from "solid-js";

export function useDraft() {
  const [input, setInput] = createSignal("");
  return {
    input,
    setDraft: (text: string) => {
      setInput(text);
    },
  };
}

export function useChat(props: { paneId: string }) {
  const { setDraft } = useDraft();
  return merge(props, { setDraft });
}
