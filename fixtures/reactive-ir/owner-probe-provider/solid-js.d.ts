declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Owner = object;
  export interface Context<T> { id: symbol; defaultValue: T }
  export function getOwner(): Owner | null;
  export function createContext<T>(defaultValue?: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
}
