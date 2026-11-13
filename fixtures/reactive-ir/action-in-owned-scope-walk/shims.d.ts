// JavaScript the program does not analyze (`allowJs` is off): each shim stands for `step.vendor.js`.
declare module "*.vendor.js" { export const vendorStep: (cb: () => void) => void; }
declare module "*.js?fz" { export const vendorStep: (cb: () => void) => void; }
