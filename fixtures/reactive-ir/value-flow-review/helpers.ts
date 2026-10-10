export function evens(list: number[]): number[] {
  return list.filter((value) => value % 2 === 0);
}

export function joined(list: string[]): string {
  return list.join(",");
}

export declare function external(): number[];
