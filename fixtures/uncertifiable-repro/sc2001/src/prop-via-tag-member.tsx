// error-menu-web AnalyzerSelect.tsx:9-50 (callback from AddProjectModal.tsx:285, ProjectSettingsModal.tsx:224): a row component renders a tag taken
// from its own props, `<properties.analyzer.icon />`, and the callback reaches it through a forwarding wrapper's `For`.
import { createSignal, For } from "solid-js";

type Analyzer = { id: string; icon: (props: { size: number }) => string };

const analyzers: Analyzer[] = [{ id: "a", icon: (props) => `icon ${props.size}` }];

const Row = (properties: { analyzer: Analyzer; onToggle: (id: string) => void }) => (
  <li>
    <properties.analyzer.icon size={14} />
    <input type="checkbox" onInput={() => properties.onToggle(properties.analyzer.id)} />
  </li>
);

const Select = (properties: { onToggle: (id: string) => void }) => (
  <ul>
    <For each={analyzers}>{(analyzer) => <Row analyzer={analyzer} onToggle={properties.onToggle} />}</For>
  </ul>
);

export function Modal() {
  const [chosen, setChosen] = createSignal<string[]>([]);
  const toggle = (id: string): void => {
    setChosen([...chosen(), id]);
  };
  return <Select onToggle={toggle} />;
}
