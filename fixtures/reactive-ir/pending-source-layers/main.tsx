import { render } from "@solidjs/web";
import {
  ContextDestructured,
  HookAfterEffect,
  HookDestructured,
  HookRunsCode,
  PropAlias,
  PropAliasLate,
  PropClosure,
  PropHelper,
  PropIife,
  PropKept,
  PropThrowFirst,
} from "./App";

const host = document.body;
render(() => <PropIife />, host);
render(() => <PropClosure />, host);
render(() => <PropHelper />, host);
render(() => <PropKept />, host);
render(() => <PropThrowFirst />, host);
render(() => <HookDestructured />, host);
render(() => <HookAfterEffect />, host);
render(() => <HookRunsCode />, host);
render(() => <PropAlias />, host);
render(() => <PropAliasLate />, host);
render(() => <ContextDestructured />, host);
