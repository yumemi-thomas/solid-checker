export async function saveEvent(when: Date) {
  "use server";
  return when.toISOString();
}
