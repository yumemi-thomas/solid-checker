export interface Client {
  getThing(): number;
}
export declare function channelFor(client: Client): number;
export declare function forwarded(props: { client: Client }): { value: number };
export declare function Isolated(): { value: number };
