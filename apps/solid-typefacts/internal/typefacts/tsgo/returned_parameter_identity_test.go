package tsgo

import (
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

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

// A throw guard before the return -- `@solidjs/web`'s `withMeta` shape -- leaves
// the return's value-return edge unconditional: an execution that throws returns
// no value, so every normal completion still passes the one return. An arm that
// may `return` instead is a competing value-return edge and keeps the
// successor's carry unknown, exactly as before.
func TestThrowGuardKeepsTheSuccessorReturnUnconditional(t *testing.T) {
	cases := []struct {
		name, declaration string
		want              typefacts.Reachability
	}{
		{
			"throwGuard",
			`function make(value: any, other: any) { if (!other) { throw new Error("guard"); } return value; }`,
			typefacts.Reachable,
		},
		{
			"throwGuardUnbraced",
			`function make(value: any, other: any) { if (!other) throw new Error("guard"); return value; }`,
			typefacts.Reachable,
		},
		{
			"elseThrowGuard",
			`function make(value: any, other: any) { if (other) { other.touch(); } else { throw new Error("guard"); } return value; }`,
			typefacts.Reachable,
		},
		{
			"returnGuard",
			`function make(value: any, other: any) { if (!other) { return value; } return value; }`,
			typefacts.ReachUnknown,
		},
		{
			"throwAfterConditionalReturn",
			`function make(value: any, other: any) { if (!other) { if (other === null) { return value; } throw new Error("guard"); } return value; }`,
			typefacts.ReachUnknown,
		},
		{
			"nestedCallableReturnIsNotAReturn",
			`function make(value: any, other: any) { if (!other) { const f = () => { return 1; }; f(); throw new Error("guard"); } return value; }`,
			typefacts.Reachable,
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			transcript := invocationTranscriptForMake(t, test.declaration+"\nmake({}, 1);\n")
			returns := transcript.ControlFlow.Returns
			if len(returns) == 0 {
				t.Fatalf("no returns: %#v", transcript.ControlFlow)
			}
			last := returns[len(returns)-1]
			if last.Reach != typefacts.Reachable {
				t.Fatalf("final return reach = %v, want reachable", last.Reach)
			}
			if last.CarryReach == nil || *last.CarryReach != test.want {
				t.Fatalf("final return carry reach = %v, want %v", last.CarryReach, test.want)
			}
		})
	}
}
