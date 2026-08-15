"use server";
// A module-level directive marks every export a server function; the calls
// in App.tsx cross the transport just the same.
export async function recordPattern(pattern: RegExp) {
  return String(pattern);
}
