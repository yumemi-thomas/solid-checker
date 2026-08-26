// Edited by the suppression-flip test purely to advance the generation. Nothing
// references it, so editing it must leave every other file retained.

const islandSeed = 7;

function islandScale(value: number): number {
  return value * islandSeed;
}

const island = islandScale(2);
