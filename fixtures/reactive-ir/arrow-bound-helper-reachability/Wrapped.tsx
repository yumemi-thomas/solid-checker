// Negative: a hoisted declaration run at module init through a callee
// wrapped in `!`, `as` or `satisfies` still runs above the declarator.
import { createRoot, createSignal } from "solid-js";
const [count, setCount] = createSignal(0);
void count;

// Each goN runs at module init, above helperN's declarator: the root callback
// throws ReferenceError and setCount never runs. Plain `go0()` is withheld.
function go0() { createRoot(() => { helper0(); }); }
go0();
const helper0 = () => { setCount(0); };

function go1() { createRoot(() => { helper1(); }); }
go1!();
const helper1 = () => { setCount(1); };

function go2() { createRoot(() => { helper2(); }); }
(go2 as () => void)();
const helper2 = () => { setCount(2); };

function go3() { createRoot(() => { helper3(); }); }
(go3 satisfies () => void)();
const helper3 = () => { setCount(3); };
