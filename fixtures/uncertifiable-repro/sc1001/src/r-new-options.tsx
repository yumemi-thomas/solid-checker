// solid-groove EditorView.tsx:206-208: a component body builds an engine whose options object
// reads a signal (`new ToneAuditionEngine(runtime, { songTempo: () => tempo() })`).
import { createSignal } from "solid-js";

class ToneEngine {
  constructor(private readonly options: { songTempo: () => number }) {}
  bpm() {
    return this.options.songTempo();
  }
}

function Roll(props: { engine: ToneEngine }) {
  return <p>{props.engine.bpm()}</p>;
}

function Editor(props: { create?: () => ToneEngine }) {
  const [tempo, setTempo] = createSignal(120);
  const createEngine = props.create ?? (() => new ToneEngine({ songTempo: () => tempo() }));
  return (
    <div>
      <input type="number" value={tempo()} onInput={(event) => setTempo(event.currentTarget.valueAsNumber)} />
      <Roll engine={createEngine()} />
    </div>
  );
}

export function App() {
  return <Editor />;
}
