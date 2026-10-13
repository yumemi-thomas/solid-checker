type Callback = (value: number) => number;
export declare class Keeper {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class Primed {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class RunsAtConstruction {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class HandsOn {
  constructor(callback: Callback);
  later(): number[];
}
export declare class Escapes {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class Rewritten {
  constructor(callback: Callback);
  reset(next: Callback): void;
  run(value: number): number;
}
export declare class Augmented {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class PrimedUndescribed {
  constructor(callback: Callback);
  run(value: number): number;
}
export declare class KeeperUnkept {
  constructor(callback: Callback);
}
