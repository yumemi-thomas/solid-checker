/** @jsxImportSource @solidjs/web */
import { createSignal, onSettled } from "solid-js";
import { Show } from "@solidjs/web";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const audio = createAudio("");
  const [ready, setReady] = createSignal(false, { ownedWrite: true });
  onSettled(() => {
    // Exercise the published listener; do not read pending duration during setup.
    audio.player.dispatchEvent(new Event("loadeddata"));
    setReady(true);
  });
  const Duration = () => {

    return <p>{String(audio.duration())}</p>;
  };
  return <Show when={ready()}><Duration /></Show>;
}
