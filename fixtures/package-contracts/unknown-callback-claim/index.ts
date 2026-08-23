declare function unknownScheduler(callback: () => void): void;

export function schedule(callback: () => void): void {
  unknownScheduler(callback);
}

export function plain(value: number): number {
  return value;
}
