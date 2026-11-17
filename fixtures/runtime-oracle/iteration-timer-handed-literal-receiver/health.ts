export interface HealthOptions {
  now(): number;
  readonly schedule: (delayMs: number, run: () => void) => void;
}

export function createHealth(options: HealthOptions): number {
  options.schedule(1000, () => {
    void 0;
  });
  return options.now();
}
