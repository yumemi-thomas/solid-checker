// skyjtx-website routes/(main)/user/[id].tsx:6,52: a client call to a "use server" function, argument typed through an object parameter.
async function loadUserData(id: number) {
  "use server";
  return { id, name: `User ${id}` };
}

export function preload(params: { id: number }) {
  return loadUserData(params.id);
}
