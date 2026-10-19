import { api, reader } from "./api";
export function patch(src: Record<string, unknown>) {
  void src;
  const k = "re" + "ad";
  (api as Record<string, unknown>)[k] = reader.read;
}
