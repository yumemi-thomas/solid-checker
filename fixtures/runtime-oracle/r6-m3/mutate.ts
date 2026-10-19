import { api, reader } from "./api";
export function patch(src: Record<string, unknown>) {
  void src;
  Object.assign(api, { read: reader.read });
}
