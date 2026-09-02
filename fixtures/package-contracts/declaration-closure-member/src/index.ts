import type { Options } from "./options.js";
import { label } from "./label.js";

export function createWidget(options: Options) {
  return { label: label(options.name) };
}
