export interface Client {
  getThing(): number;
}
export declare function Panel(props: { client: Client }): { value: number };
export declare function Root(props: { client: Client }): { value: number };
export declare function UseChannel(props: { client: Client }): { value: number };
export declare function Render(input: number): { text: string };
export declare function Isolated(): { value: number };
