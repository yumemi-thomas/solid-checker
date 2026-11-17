export interface HealthOptions {
  readonly schedule: (delayMs: number, run: () => void) => void;
}

const kept: HealthOptions[] = [];
function keep(options: HealthOptions): void {
  kept.push(options);
}

export function createHealth(options: HealthOptions): void {
  keep(options);
  options.schedule(1000, () => {
    void 0;
  });
}
