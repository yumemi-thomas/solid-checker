// F4: a package the program loads depends on vite by its manifest, and may load Vite's own code where the program runs.
import { value } from "fz-vite-dependent";

export const used = value;
