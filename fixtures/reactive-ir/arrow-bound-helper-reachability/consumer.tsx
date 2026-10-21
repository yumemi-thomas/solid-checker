import { acrossModules } from "./provider";

// Positive: provider.ts imports nothing back, so it finished evaluating before
// this module's body ran and `acrossModules` is initialized.
export function AcrossModules() {
  acrossModules();
  return <button>across</button>;
}
