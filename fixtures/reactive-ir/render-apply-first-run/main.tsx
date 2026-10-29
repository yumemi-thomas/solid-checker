import { hydrate, render } from "@solidjs/web";
import {
  ComputeReads,
  ConditionalEffect,
  FirstRunWrite,
  FirstRunWriteLayered,
  Hydrated,
  InClassField,
  OptionsVariable,
  ComputeImports,
  ComputeThenable,
  ComputeDestructures,
  Replaced,
  UnderCondition,
  RenderedLater,
  Scheduled,
} from "./App";

render(() => <FirstRunWrite />, document.body);
render(() => <FirstRunWriteLayered />, document.body);
render(() => <Scheduled />, document.body);
render(() => <OptionsVariable />, document.body);
render(() => <ComputeReads />, document.body);
render(() => <ConditionalEffect />, document.body);
hydrate(() => <Hydrated />, document.body);
function mountLater() {
  render(() => <RenderedLater />, document.body);
}
mountLater();
render(() => <ComputeImports />, document.body);
render(() => <ComputeThenable />, document.body);
render(() => <ComputeDestructures />, document.body);
render(() => <Replaced />, document.body);
class Mounts {
  mounted = render(() => <InClassField />, document.body);
}
void Mounts;
declare const ready: boolean;
if (ready) render(() => <UnderCondition />, document.body);
