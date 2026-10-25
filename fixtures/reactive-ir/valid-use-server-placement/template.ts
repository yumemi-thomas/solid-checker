// A template literal is never a directive — finding, in a function body and at
// module top.
`use server`;

export async function inFunction(id: string) {
  `use server`;
  return id;
}

// Negative: a template with a substitution is not the directive text.
export async function withSubstitution(id: string) {
  `use ${id}`;
  return id;
}
