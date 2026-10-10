import { a } from "./cycle-a.ts";
console.log(a);
export const b = 2;
void "cyclic dependency remains baseline";
