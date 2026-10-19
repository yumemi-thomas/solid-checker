import { api, reader } from "./api";
export function patch(src: Record<string, unknown>) {
  void src;
  const alias = api;
  alias.read = reader.read;
}
