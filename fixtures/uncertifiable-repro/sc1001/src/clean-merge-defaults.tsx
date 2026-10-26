// Clean. Mirrors kui frontend/packages/kernel/src/components/IconTile.tsx:34-42: merge() fills prop
// defaults; the merged props are read in the JSX class array, never destructured.
import { merge } from "solid-js";

function Tile(props: { tone?: "neutral" | "warn"; size?: "sm" | "md"; class?: string; icon: string }) {
  const p = merge({ tone: "neutral", size: "md" } as const, props);
  return (
    <span class={["tile", `tile--${p.tone}`, `tile--${p.size}`, props.class]} aria-hidden="true">
      {props.icon}
    </span>
  );
}

export function App() {
  return (
    <>
      <Tile icon="a" />
      <Tile icon="b" tone="warn" size="sm" class="extra" />
    </>
  );
}
