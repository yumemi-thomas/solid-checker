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
