import { api, reader } from "./api";
export function patch(src: Record<string, unknown>) {
  void src;
  api.read = reader.read;
}
