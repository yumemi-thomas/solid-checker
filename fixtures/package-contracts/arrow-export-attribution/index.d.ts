export interface Client {
  getThing(): number;
}
export declare const Arrowed: (props: { client: Client }) => { value: number };
export declare function Declared(props: { client: Client }): { value: number };
export declare const Direct: (props: { client: Client }) => { value: number };
export declare function Isolated(): { value: number };
