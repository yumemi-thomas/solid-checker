import { onCleanup } from "solid-js";
function enterModule() { moduleTimer.moduleTicker(); }
try { enterModule(); } catch {}
const moduleTimer = { moduleTicker() { onCleanup(() => {}); } };
