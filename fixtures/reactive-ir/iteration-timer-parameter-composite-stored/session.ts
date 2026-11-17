let slot: ((resume: () => void) => void) | undefined;

function afterNextPaint(resume: () => void): void {
  setTimeout(resume, 0);
}

export function startBrowse(): void {
  (slot = afterNextPaint)(() => {
    void 0;
  });
}

export function later(callback: () => void): void {
  slot?.(callback);
}
