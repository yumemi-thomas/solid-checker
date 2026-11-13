export interface Calm {
  run(cb: () => void): number;
}
export declare const makeCalm: () => Calm;
