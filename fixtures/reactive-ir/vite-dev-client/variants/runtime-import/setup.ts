// A program file loads `vite` at run time (a test setup, a script): Vite's own code may run where the program does.
import { install } from "vite";

install("fzprobe", 1);
