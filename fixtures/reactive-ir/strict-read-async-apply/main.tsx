import { render } from "@solidjs/web";
import {
  ApplyComputeReadsAsync,
  ApplyComputeWaitsOnOtherAsync,
  ApplyReadsAsync,
  ApplyReadsAsyncRendered,
  ApplyReadsSync,
  BodyReadsAsync,
  BodyReadsFirst,
  ReadAfterCall,
  SharedClosure,
  RenderApplyReadsAsync,
  RenderApplyReadsAsyncRendered,
} from "./App";

// ADR 0504: each component mounted by its own module-scope render.
render(() => <ApplyReadsAsync />, document.body);
render(() => <RenderApplyReadsAsync />, document.body);
render(() => <RenderApplyReadsAsyncRendered />, document.body);
render(() => <ApplyReadsAsyncRendered />, document.body);
render(() => <ApplyComputeReadsAsync />, document.body);
render(() => <ApplyComputeWaitsOnOtherAsync />, document.body);
render(() => <ApplyReadsSync />, document.body);
render(() => <BodyReadsAsync />, document.body);
render(() => <SharedClosure />, document.body);
render(() => <ReadAfterCall />, document.body);
render(() => <BodyReadsFirst />, document.body);
