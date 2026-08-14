declare function unknownScheduler(callback: () => void): void;

export function unrelated(callback: () => void): void {
  unknownScheduler(callback);
}
