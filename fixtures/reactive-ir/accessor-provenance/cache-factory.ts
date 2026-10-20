import { helper } from "./cache-helper";
export function makeCache(): { ready: () => number } {
  const ready = () => { helper(); return 1; };
  return { ready };
}
