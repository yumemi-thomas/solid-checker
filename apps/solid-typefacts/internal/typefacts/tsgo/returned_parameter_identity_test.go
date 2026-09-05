package tsgo

import "testing"

func TestReturnedParameterIdentityRequiresAnUnchangedWholeBinding(t *testing.T) {
	cases := []struct {
		name, declaration string
		want              int
	}{
		{"direct", `function make(value: any, other: any) { return value; }`, 0},
		{"second", `function make(value: any, other: any) { return other; }`, 1},
		{"wrapped", `function make(value: any, other: any) { return (value as unknown); }`, 0},
		{"alias", `function make(value: any, other: any) { const alias = value; return alias; }`, -1},
		{"assigned", `function make(value: any, other: any) { value = other; return value; }`, -1},
		{"nestedWrite", `function make(value: any, other: any) { function mutate() { value = other; } return value; }`, -1},
		{"shadowedWrite", `function make(value: any, other: any) { function mutate(value: any) { value = other; } return value; }`, 0},
		{"default", `function make(value: any = 1, other: any) { return value; }`, -1},
		{"rest", `function make(...value: any[]) { return value; }`, -1},
		{"destructured", `function make({value}: any, other: any) { return value; }`, -1},
		{"async", `async function make(value: any, other: any) { return value; }`, -1},
		{"generator", `function* make(value: any, other: any) { return value; }`, -1},
		{"arguments", `function make(value: any, other: any) { arguments[0] = other; return value; }`, -1},
		{"eval", `function make(value: any, other: any) { eval("value = other"); return value; }`, -1},
		{"initializerEval", `function make(value: any, other: any = eval("value = 1")) { return value; }`, -1},
		{"duplicateParameter", `function make(value: any, value: any) { return value; }`, -1},
		{"property", `function make(value: any, other: any) { return value.child; }`, -1},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			transcript := invocationTranscriptForMake(t, test.declaration+"\nmake({}, 1);\n")
			if len(transcript.ControlFlow.Returns) != 1 {
				t.Fatalf("returns = %#v", transcript.ControlFlow.Returns)
			}
			got := transcript.ControlFlow.Returns[0].Parameter
			if test.want < 0 {
				if got != nil {
					t.Fatalf("unexpected identity: %#v", got)
				}
			} else if got == nil || got.ParameterIndex != test.want || len(got.Path) != 0 {
				t.Fatalf("identity = %#v, want whole parameter %d", got, test.want)
			}
		})
	}
}
