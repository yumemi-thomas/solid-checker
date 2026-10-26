// Clean. Mirrors hoxxes-briefing apps/web/src/pages/briefing/ui/dive/dive-glyphs.tsx:171-178: omit() drops
// the lookup key, and the rest is spread onto the component a keyed Show picks.
import { omit, Show } from "solid-js";

type IconProps = { size?: number; label?: string };
const glyphs: Record<string, string> = { eliminate: "M0 0L8 8", mine: "M4 0L4 8" };

function Glyph(props: IconProps & { d: string }) {
  return <svg width={props.size ?? 16} aria-label={props.label}><path d={props.d} /></svg>;
}

function Fallback(props: IconProps) {
  return <svg width={props.size ?? 16} aria-label={props.label} />;
}

function KindIcon(props: IconProps & { kind: string }) {
  const rest = omit(props, "kind");
  return (
    <Show when={glyphs[props.kind]} fallback={<Fallback {...rest} />} keyed>
      {(d) => <Glyph d={d} {...rest} />}
    </Show>
  );
}

export function App() {
  return (
    <>
      <KindIcon kind="eliminate" size={20} />
      <KindIcon kind="unknown" label="none" />
    </>
  );
}
