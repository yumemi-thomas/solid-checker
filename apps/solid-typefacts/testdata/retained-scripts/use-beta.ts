const betaSeed = scale(factor);

async function run_beta(): Promise<number> {
  const value = await loadValue();
  return scale(value + betaSeed);
}
