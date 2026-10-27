// solid-groove useLibraryBrowser.ts:151-153: a callback in the options object of a project class
// constructor (`new AuditionController(engine, { onActiveChange })`) writes.
import { createSignal } from "solid-js";

class AuditionController {
  constructor(private readonly options: { onActiveChange: (id: string | null) => void }) {}
  play(id: string) {
    this.options.onActiveChange(id);
  }
}

export function createLibraryBrowser() {
  const [auditioningId, setAuditioningId] = createSignal<string | null>(null);
  const audition = new AuditionController({ onActiveChange: (id) => setAuditioningId(id) });
  return { auditioningId, play: (id: string) => audition.play(id) };
}
