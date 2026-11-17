interface HealthOptions {
  readonly schedule: (delayMs: number, run: () => void) => void;
}
function createHealth(options: HealthOptions): void {
  options.schedule(1000, () => { void 0; });
}
