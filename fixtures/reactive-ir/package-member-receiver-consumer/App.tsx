import { createPanel, createTicker } from "reactive-package";

function keep(value: unknown) { return value; }
function View(_props: { value: unknown }) { return <div />; }

// A mandatory read is a violation; an optional read is uncertifiable.
export function MandatoryInBody() {
  const panel = createPanel();
  panel.stop();
  return <div />;
}
export function OptionalInBody() {
  const panel = createPanel();
  const frozen = panel.value();
  return <div>{String(frozen)}</div>;
}
export function OptionalInJsx() {
  const panel = createPanel();
  return <div>{String(panel.value())}</div>;
}
export function MandatoryInJsx() {
  const panel = createPanel();
  return <div>{String(panel.stop())}</div>;
}
export function NoRead() {
  const panel = createPanel();
  panel.idle();
  return <div />;
}
export function MemberInHandler() {
  const panel = createPanel();
  return <div onClick={() => panel.stop()} />;
}
export function DestructuredOptional() {
  const { value } = createPanel();
  const frozen = value();
  return <div>{String(frozen)}</div>;
}
export function DestructuredOptionalInJsx() {
  const { value } = createPanel();
  return <div>{String(value())}</div>;
}
export function MutableReceiver() {
  let panel = createPanel();
  panel.stop();
  return <div />;
}
export function InArray() {
  const panel = createPanel();
  keep([panel]);
  panel.stop();
  return <div />;
}
export function InShorthand() {
  const panel = createPanel();
  keep({ panel });
  panel.stop();
  return <div />;
}
export function CastEscape() {
  const panel = createPanel();
  keep(panel as unknown);
  panel.stop();
  return <div />;
}
export function AliasEscape() {
  const panel = createPanel();
  const alias = panel;
  keep(alias);
  panel.stop();
  return <div />;
}
export function PassedReceiver() {
  const panel = createPanel();
  keep(panel);
  panel.stop();
  return <div />;
}
export function ReturnedReceiver() {
  const panel = createPanel();
  return panel;
}
export const exportedReceiver = createPanel();
exportedReceiver.stop();
const namedExport = createPanel();
export { namedExport };
namedExport.stop();
export function InJsxAttribute() {
  const panel = createPanel();
  panel.stop();
  return <View value={panel} />;
}
export function WrappedCallee() {
  const panel = createPanel();
  (panel.stop as () => void)();
  return <div />;
}
export function SatisfiesCallee() {
  const panel = createPanel();
  (panel.stop satisfies () => void)();
  return <div />;
}
export function ParenthesizedCallee() {
  const panel = createPanel();
  (panel.stop)();
  return <div />;
}
export function WrappedReceiver() {
  const panel = createPanel();
  (panel as ReturnType<typeof createPanel>).stop();
  return <div />;
}
export function WrappedInitializer() {
  const panel = createPanel() as ReturnType<typeof createPanel>;
  panel.stop();
  return <div />;
}
export function ComputedSelection() {
  const panel = createPanel();
  panel["stop"]();
  return <div />;
}
export function MemberEscape() {
  const panel = createPanel();
  keep(panel.stop);
  return <div />;
}
export function Mutation() {
  const panel = createPanel();
  panel.stop = () => {};
  panel.stop();
  return <div />;
}
export function OpaqueCall() {
  const panel = createPanel();
  panel.opaque();
  panel.stop();
  return <div />;
}
export function ShadowedReceiver() {
  const panel = createPanel();
  function inner(panel: { stop: () => void }) { panel.stop(); }
  keep(inner);
  return <div>{String(panel.value())}</div>;
}
// Tuple selection is deliberately outside this object-only slice.
export function TupleSelection() {
  const ticker = createTicker();
  ticker[1]();
  return <div />;
}
