import { Panel, Panel as Aliased, Quiet, Plain } from "dep-ui-context";
import type { Panel as PanelType } from "dep-ui-context";
import * as ui from "dep-ui-context";

type PanelProps = Parameters<typeof Panel>[0];
const LocalAlias = Panel;
function Inner() {
  return <Panel>nested</Panel>;
}

export function Direct() {
  return <Panel>x</Panel>;
}
export function Renamed() {
  return <Aliased>x</Aliased>;
}
export function Namespace() {
  return <ui.Panel>x</ui.Panel>;
}
export function ViaLocalAlias() {
  return <LocalAlias>x</LocalAlias>;
}
export function Nested() {
  return <Inner />;
}
export function Defaulted() {
  return <Quiet>x</Quiet>;
}
export function NoRead() {
  return <Plain>x</Plain>;
}
export function TypeOnly(props: PanelProps) {
  const kind: typeof PanelType | undefined = undefined;
  return <p>{String(kind)}</p>;
}
