interface Values {
  values(): unknown;
}
export declare function captured(props: { of: Values }): void;
export declare function direct(props: { of: Values }): void;
export declare function mixed(props: { of: Values; other: Values }): void;
