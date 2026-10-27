// Clean. Mirrors spotify-desk-thing src/components/PlayerControls/PlayerProgressBar.tsx:24-35: an effect's
// apply starts an interval that writes a signal functionally and returns the teardown that stops it.
import { createEffect, createMemo, createSignal } from "solid-js";

export function Progress() {
  const [progressMs, setProgressMs] = createSignal(0);
  const [playing, setPlaying] = createSignal(true);
  createEffect(playing, (isPlaying) => {
    let progressInterval: ReturnType<typeof setInterval> | undefined;
    if (isPlaying) {
      progressInterval = setInterval(() => setProgressMs((current) => current + 1000), 20);
    }
    return () => {
      clearInterval(progressInterval);
    };
  });
  const ratio = createMemo(() => progressMs() / 60_000);
  return (
    <button type="button" onClick={() => setPlaying(!playing())} style={{ width: `${ratio() * 100}%` }}>
      {progressMs()}
    </button>
  );
}
