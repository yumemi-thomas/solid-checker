"use server";
// A module-level directive file: its exports are server functions, and a
// reference here is server code (negative).
export async function archive(when: Date) {
  return when.getTime();
}
const kept = archive;
export { kept };
