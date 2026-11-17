export interface HealthOptions {
  readonly schedule: (delayMs: number, run?: (this: Window) => void) => void;
}
export function createHealth(options: HealthOptions): void {
  options.schedule(1000);
}
