// The component's runtime JavaScript is a file the program does not analyze (`allowJs` is off): this shim stands for it.
declare module "*.vendor.js" {
  const View: () => unknown;
  export default View;
}
