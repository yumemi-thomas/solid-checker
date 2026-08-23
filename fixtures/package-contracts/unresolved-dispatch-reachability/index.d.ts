export interface Client {
  getThing(): number;
}
export declare function Reaches(props: { client: Client }): { value: number };
export declare function Isolated(): { value: number };
