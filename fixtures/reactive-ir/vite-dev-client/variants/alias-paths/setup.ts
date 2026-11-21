// Review F1: "v" is spelled neither `vite` nor `vite/*`, and resolves (tsconfig `paths`; a Vite alias) to vite's Node entry.
import { install } from "v";

install("fzprobe", 1);
