function afterNextPaint(resume: () => void): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const run = (): void => {
    timer = setTimeout(resume, 0);
  };
  const frame = requestAnimationFrame(run);
  return () => {
    cancelAnimationFrame(frame);
    if (timer !== undefined) clearTimeout(timer);
  };
}

export function startBrowse(options: { scheduleAfterPaint?: (resume: () => void) => () => void }): () => void {
  return (options.scheduleAfterPaint ?? afterNextPaint)(() => {
    void 0;
  });
}
