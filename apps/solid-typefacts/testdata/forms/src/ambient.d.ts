declare module "ambient-host" {
  import { value } from "pkg";
  import { local } from "./local";
  export const hosted: typeof value;
}
