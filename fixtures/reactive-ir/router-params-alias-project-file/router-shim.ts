export function useParams<T extends Record<string, unknown>>(): T {
  return { clusterId: { value: "x" } } as unknown as T;
}
