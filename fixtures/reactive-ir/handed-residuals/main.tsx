import { render } from "@solidjs/web";
import { DirectRunShow, HookJsxShow, HookMemberShow, HookRunShow } from "./App";

// ADR 0504's mount proof: each control-flow case mounted by its own render.
render(() => <HookRunShow />, document.body);
render(() => <HookMemberShow />, document.body);
render(() => <HookJsxShow />, document.body);
render(() => <DirectRunShow />, document.body);
