const gammaSeed = scale(factor);

async function run_gamma(): Promise<number> {
  const value = await loadValue();
  return scale(value + gammaSeed);
}
