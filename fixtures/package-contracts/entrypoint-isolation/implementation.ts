declare function unknownScheduler(callback: () => void): void;

export function readRoot(): number {
  return 1;
}

export function hiddenScheduler(callback: () => void): void {
  unknownScheduler(callback);
}
