import { render } from "@solidjs/web";
import {
  ContextDestructured,
  HookAfterEffect,
  HookDestructured,
  HookRunsCode,
  LoadingHolder,
  LoadingOn,
  PropAlias,
  PropAliasLate,
  PropClosure,
  PropHelper,
  PropIife,
  PropKept,
  PropThrowFirst,
  PropWrapper,
  PropWrapperTwice,
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
render(() => <LoadingHolder />, host);
render(() => <LoadingOn />, host);
render(() => <PropWrapper />, host);
render(() => <PropWrapperTwice />, host);
