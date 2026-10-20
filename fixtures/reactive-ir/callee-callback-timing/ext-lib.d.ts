// A package seen only through its declarations: no runtime bytes, no contract.
declare module "ext-lib" {
  export function subscribe(callback: () => void): void;
}
