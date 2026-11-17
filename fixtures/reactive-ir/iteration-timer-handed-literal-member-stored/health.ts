export interface HealthOptions {
  readonly schedule: (delayMs: number, run: (this: Window) => void) => void;
}
export function createHealth(options: HealthOptions): void {
  let saved: HealthOptions["schedule"] | undefined;
  (saved = options.schedule)(1000, () => { void 0; });
  saved(1000, function () { void this; });
}
