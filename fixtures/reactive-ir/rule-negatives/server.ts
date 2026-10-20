"use server";

// server-function-module-directive (SC7006): a "use server" module whose
// exports are direct function declarations and function expressions.
export async function addTodo(title: string) {
  return title.length;
}
export const removeTodo = async (id: string) => id.length;
