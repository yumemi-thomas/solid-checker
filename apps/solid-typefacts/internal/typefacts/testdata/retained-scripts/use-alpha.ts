const alphaSeed = scale(factor);

async function run_alpha(): Promise<number> {
  const value = await loadValue();
  return scale(value + alphaSeed);
}
