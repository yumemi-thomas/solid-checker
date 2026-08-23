export interface Client {
  getThing(): number;
}
export declare function inert(onReady: (value: number) => void): number;
export declare function Direct(
  props: { client: Client },
  onReady: (value: number) => void
): { value: () => number };
