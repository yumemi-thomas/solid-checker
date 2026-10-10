import { OBSERVE, createRoot, getOwner } from "solid-js";
createRoot(dispose => { if (OBSERVE) OBSERVE.exclude(getOwner()!); dispose(); });
