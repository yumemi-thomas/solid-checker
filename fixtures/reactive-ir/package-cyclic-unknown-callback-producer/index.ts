declare function unknownScheduler(callback: () => void): void;

export function schedule(callback: () => void): void {
  forward(callback);
}

function forward(callback: () => void): void {
  schedule(callback);
  unknownScheduler(callback);
}
