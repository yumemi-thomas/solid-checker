export interface Client {
  getThing(): number;
}
export declare function Direct(props: { client: Client }): { value: number };
export declare function Isolated(): { value: number };
