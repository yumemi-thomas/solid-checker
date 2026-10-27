// Clean. Mirrors expenses-app src/components/icons.tsx:31-33: a For over [tag, attrs] pairs renders each
// as a Dynamic component with the attributes spread, inside an svg.
import { For } from "solid-js";
import { Dynamic } from "@solidjs/web";

function Icon(props: { icon: [string, Record<string, string>][]; size?: number }) {
  const size = () => props.size ?? 24;
  return (
    <svg width={size()} height={size()} viewBox="0 0 24 24" fill="none" stroke="currentColor">
      <For each={props.icon}>{([tag, attrs]) => <Dynamic component={tag} {...attrs} />}</For>
    </svg>
  );
}

export function App() {
  return <Icon icon={[["path", { d: "M0 0L10 10" }], ["circle", { cx: "5", cy: "5", r: "2" }]]} />;
}
