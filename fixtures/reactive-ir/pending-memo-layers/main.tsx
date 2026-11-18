import { render } from "@solidjs/web";
import { ApplyClosure, MemoClosure, MemoClosureHanded, MemoClosureHelper, MemoClosureKept, MemoHelper, MemoIife, MemoUntrackClosure } from "./App";

const host = document.body;
render(() => <MemoClosure />, host);
render(() => <MemoIife />, host);
render(() => <MemoHelper />, host);
render(() => <MemoClosureHelper />, host);
render(() => <ApplyClosure />, host);
render(() => <MemoUntrackClosure />, host);
render(() => <MemoClosureKept />, host);
render(() => <MemoClosureHanded />, host);
