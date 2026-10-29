import { hydrate, render } from "@solidjs/web";
import {
  ComputeReads,
  ConditionalEffect,
  FirstRunWrite,
  FirstRunWriteLayered,
  Hydrated,
  OptionsVariable,
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
