export interface Client {
  getThing(cb: (item: number) => void): void;
}
export declare function subscribe(client: Client, cb: (item: number) => void): void;
export declare const PREFIX: string;
