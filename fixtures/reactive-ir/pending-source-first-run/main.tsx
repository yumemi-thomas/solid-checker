import { render } from "@solidjs/web";
import {
  CallingParent,
  ContextParent,
  LateParent,
  ModuleRender,
  ModuleSecond,
  PropParent,
  ThrowBeforeWrite,
  ThrowInHelper,
  ThrowInIife,
  ThrowInWrapper,
  WrittenParent,
} from "./App";

const host = document.body;
render(() => <ModuleRender />, host);
render(() => <ModuleSecond />, host);
render(() => <PropParent />, host);
render(() => <CallingParent />, host);
render(() => <ContextParent />, host);
render(() => <WrittenParent />, host);
render(() => <LateParent />, host);
render(() => <ThrowInIife />, host);
render(() => <ThrowInWrapper />, host);
render(() => <ThrowInHelper />, host);
render(() => <ThrowBeforeWrite />, host);
