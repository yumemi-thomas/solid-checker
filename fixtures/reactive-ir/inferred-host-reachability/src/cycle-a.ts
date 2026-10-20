import { b } from "./cycle-b.ts";
export const a = 1;
console.log(b);
void "cyclic entry remains baseline";
