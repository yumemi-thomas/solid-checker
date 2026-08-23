export interface Client {
  getThing(): number;
}
export declare function Inline(onData: (value: number) => void): { done: boolean };
export declare function Escaping(
  props: { client: Client },
  onData: (value: number) => void
): { value: number };
export declare function Returned(onData: (value: number) => void): { run: () => void };
