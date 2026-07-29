declare function unknownScheduler(callback: () => void): void;

export function schedule(callback: () => void): void {
  unknownScheduler(callback);
}

export function invokeReflectively(callback: () => void): void {
  Reflect.apply(callback, undefined, []);
}
