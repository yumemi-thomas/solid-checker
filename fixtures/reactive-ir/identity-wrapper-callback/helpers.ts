export function wrapImported(callback: () => void): () => void {
  return callback;
}
