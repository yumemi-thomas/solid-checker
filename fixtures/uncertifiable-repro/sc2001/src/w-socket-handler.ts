// inferay useSpeechToText.tsx:182-188: a callback assigned to an `on…` property of a browser
// object (`recognition.onerror = …`) writes.
import { createSignal } from "solid-js";

export function createSocketStatus(url: string) {
  const [message, setMessage] = createSignal("");
  const socket = new WebSocket(url);
  socket.onmessage = (event) => {
    setMessage(String(event.data));
  };
  return message;
}
