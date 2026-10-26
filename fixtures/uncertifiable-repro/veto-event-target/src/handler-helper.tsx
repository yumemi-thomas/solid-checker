// app-game folder-tabs App.tsx:41, solid-svg-editor InspectorInputs.tsx:463 (toggleRelative), browser-atlas
// App.tsx:2223: handlers call a helper that invokes a method on a value they hand it (`event.preventDefault()`,
// `commands.map(...)`, `backend.load()`). The handlers are placed only while no dispatch site of the project
// withholds that proof (ADR 0356), here dispatch-sites.ts's.
import { createSignal } from "solid-js";

type Command = { relative: boolean; x: number };
interface Backend {
  load(): Promise<string>;
}

function navigate(event: MouseEvent, href: string): void {
  event.preventDefault();
  history.pushState(null, "", href);
}

function toggleRelative(commands: Command[], index: number): Command[] {
  return commands.map((command, at) => (at === index ? { ...command, relative: !command.relative } : command));
}

async function snapshot(backend: Backend, label: string): Promise<string> {
  return `${label}:${await backend.load()}`;
}

export function Editor(props: { href: string; backend: Backend }) {
  const [commands, setCommands] = createSignal<Command[]>([]);
  return (
    <div>
      <a href={props.href} onClick={(event) => navigate(event, props.href)}>
        Open
      </a>
      <button onClick={() => setCommands(toggleRelative(commands(), 0))}>Toggle</button>
      <button onClick={() => void snapshot(props.backend, "export")}>Export</button>
    </div>
  );
}
