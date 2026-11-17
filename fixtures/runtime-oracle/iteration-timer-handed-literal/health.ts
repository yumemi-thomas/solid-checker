export interface HealthOptions {
  readonly now: () => number;
  readonly schedule: (delayMs: number, run: () => void) => void;
}

export function createHealth(options: HealthOptions): () => number {
  options.schedule(1000, () => {
    void options.now();
  });
  return () => options.now();
}
