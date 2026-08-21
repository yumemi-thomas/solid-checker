// Function-level "use server" server functions: their call sites elsewhere in
// the project go through the client transport, which sends argument lists as
// plain JSON unless enableRichArguments() installs the codec.
export async function saveEvent(when: Date, tags: Set<string>) {
  "use server";
  return { when, tags };
}

export async function saveCounts(counts: Map<string, number>) {
  "use server";
  return counts.size;
}

export async function uploadChunk(bytes: Uint8Array) {
  "use server";
  return bytes.length;
}

export async function appendChunk(name: string, bytes: Uint8Array) {
  "use server";
  return name.length + bytes.length;
}

export async function analyze(samples: Float64Array, label: string) {
  "use server";
  return label.length + samples.length;
}

export async function savePlain(payload: { title: string }) {
  "use server";
  return payload.title;
}

export type SafeScalar = string | boolean | null | 0 | 1.5;

export async function saveScalar(value: SafeScalar) {
  "use server";
  return value;
}

export type UnsafeScalar = bigint | symbol | undefined;

export async function saveUnsafeScalar(value: UnsafeScalar) {
  "use server";
  return typeof value;
}

export async function saveNumber(value: number) {
  "use server";
  return value;
}

// The alias cases. Each parameter's type renders as its own name, so the text
// walk this rule used to do matched nothing on any of them.
export async function saveStamps(stamps: Stamps) {
  "use server";
  return stamps.length;
}

export async function saveIds(ids: Ids) {
  "use server";
  return ids.size;
}

export async function saveBoxed(box: Boxed) {
  "use server";
  return box.title;
}

export type Stamps = Date[];
export type Ids = Set<string>;
export type Boxed = { title: string; when: Date };
