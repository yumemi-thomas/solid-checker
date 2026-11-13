export interface Kernel {
  run(cb: () => void): void;
}
export declare const makeKernel: () => Kernel;
