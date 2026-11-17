// The global object handed to code the checker does not see.
declare function unseen(value: unknown): void;
export function hand() {
  unseen(window);
}
