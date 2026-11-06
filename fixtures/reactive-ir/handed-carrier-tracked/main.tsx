import { render } from "@solidjs/web";
import { App as Apply } from "./g2d-apply";
import { App as Map } from "./g2d-map";
import { App as Module } from "./g2d-module";
import { App as Sort } from "./g2f-sort";
import { App as UnderLoading } from "./g2a";
import { App as ZeroRun } from "./g2b";

render(
  () => (
    <div>
      <Apply />
      <Map />
      <Module />
      <Sort />
      <UnderLoading />
      <ZeroRun />
    </div>
  ),
  document.body,
);
