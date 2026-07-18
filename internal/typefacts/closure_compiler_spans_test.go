package typefacts

import "testing"

func TestCompilerSpanIdentifiersAreBoundedAndStructural(t *testing.T) {
	source := []byte(`return <button onClick={() => count()}>Read</button>`)
	start := len(`return <button `)
	end := len(source) - len(`>Read</button>`)
	got := compilerSpanIdentifiers("/App.tsx", source, Location{
		Path: "/App.tsx", StartByte: start, EndByte: end,
	})
	var names []string
	for _, location := range got {
		names = append(names, string(source[location.StartByte:location.EndByte]))
	}
	want := []string{"onClick", "count"}
	if len(got) != len(want) {
		t.Fatalf("identifiers = %v, want %v", names, want)
	}
	for index := range want {
		if names[index] != want[index] {
			t.Fatalf("identifiers = %v, want %v", names, want)
		}
	}
}

func TestCompilerSpanIdentifiersRejectInvalidRange(t *testing.T) {
	if got := compilerSpanIdentifiers("/a.ts", []byte("value"), Location{
		Path: "/a.ts", StartByte: 4, EndByte: 9,
	}); got != nil {
		t.Fatalf("invalid range produced %#v", got)
	}
}
