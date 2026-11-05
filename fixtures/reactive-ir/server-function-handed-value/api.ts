// Function-level server functions, read as values in App.tsx.
export async function save(when: Date) {
  "use server";
  return when.getTime();
}

// Negative: a reference inside another server function is a server-side call.
export async function saveTwice(when: Date) {
  "use server";
  const again = save;
  return again(when);
}
