export interface Client {
  getThing(): number;
}
export declare function inert(onReady: (value: number) => void): number;
export declare function Direct(
  props: { client: Client },
  onReady: (value: number) => void
): { value: number };
export declare function Arrow(
  props: { client: Client },
  onReady: (value: number) => void
): { read: () => number };
export declare function Helper(
  props: { client: Client },
  onReady: (value: number) => void
): { compute: () => number };
