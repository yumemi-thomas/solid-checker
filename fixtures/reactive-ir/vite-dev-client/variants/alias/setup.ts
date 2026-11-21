// Review F1: only the Vite alias sends "v" to vite; TypeScript knows the module by its ambient declaration (`v.d.ts`).
import { install } from "v";

install("fzprobe", 1);
