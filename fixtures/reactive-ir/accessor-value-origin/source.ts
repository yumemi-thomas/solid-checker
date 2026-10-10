export function evens(list: number[]): number[] {
  return list.filter((value) => value % 2 === 0);
}

export function parse(text: string): number[] {
  return text.split(",").map(Number);
}

export function same(list: number[]): number[] {
  return list;
}
