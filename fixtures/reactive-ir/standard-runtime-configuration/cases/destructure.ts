import { DEV } from "@solidjs/signals";
if (DEV) { const { hooks } = DEV; hooks.onUpdate = () => {}; }
