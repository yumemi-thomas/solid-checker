// A package component the program has only declarations for: nothing says
// what it does with its callback prop.
declare module "external-ui" {
  export function Opaque(props: { onSelect?: (value: unknown) => void }): JSX.Element;
}
