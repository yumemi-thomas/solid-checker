// Additional selections for the offline runner; the historical breadth cohort
// remains unchanged unless these packages are explicitly requested.
export const parameterPassthroughCases = [
  { name: "i18n", version: "3.0.0-next.4",
    targets: ["template", "identityResolveTemplate", "missingKeyAsPath"],
    body: 'template("hello"); identityResolveTemplate("hello"); missingKeyAsPath("hello");',
    callbackBody: 'identityResolveTemplate(`${read()}`);' }
];
