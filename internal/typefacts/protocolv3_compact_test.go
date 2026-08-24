package typefacts

import (
	"bytes"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/wirecbor"
)

func TestCompactDemandsRoundTrip(t *testing.T) {
	demands := []EntityDemand{
		{
			Location: Location{Path: "/p/a.tsx", StartByte: 1, EndByte: 4},
			Symbol:   true, References: true,
		},
		{
			Location:      Location{Path: "/p/a.tsx", StartByte: 5, EndByte: 9},
			QueryLocation: &Location{Path: "/p/a.tsx", StartByte: 6, EndByte: 8},
			Symbol:        true, TypeDescriptor: true, ResolvedCall: true,
		},
		{
			Location: Location{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
			Async:    true, StructuralAccessor: true,
		},
		{
			// A cross-file query location with every honoured flag set.
			Location:             Location{Path: "/p/b.tsx", StartByte: 9, EndByte: 12},
			QueryLocation:        &Location{Path: "/p/other.tsx", StartByte: 0, EndByte: 2},
			Symbol:               true,
			References:           true,
			TypeDescriptor:       true,
			ResolvedCall:         true,
			Callability:          true,
			Constructability:     true,
			RuntimeValueDomain:   true,
			PrimitiveValueDomain: true,
			CallResultDomain:     true,
			ConstantValue:        true,
			ArrayShape:           true,
			TupleShape:           true,
			LibraryTypes:         true,
			ReferenceSpace:       true,
			RuntimeIdentity:      true,
		},
	}
	compact := CompactDemandsV3From(demands)
	encoded, err := wirecbor.Marshal(compact)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CompactDemandsV3
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	expanded, err := decoded.Expand()
	if err != nil {
		t.Fatal(err)
	}
	if cap(expanded) != len(expanded) {
		t.Fatalf("expanded demand capacity = %d, want exact %d", cap(expanded), len(expanded))
	}
	expectedBytes, err := wirecbor.Marshal(demands)
	if err != nil {
		t.Fatal(err)
	}
	actualBytes, err := wirecbor.Marshal(expanded)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(expectedBytes, actualBytes) {
		t.Fatal("expanded demands differ from original on the wire")
	}
}

func TestCompactExpansionFailsClosedOnStringGaps(t *testing.T) {
	demands := CompactDemandsV3{
		Strings: []string{""},
		Groups:  []CompactDemandGroupV3{{Path: 9}},
	}
	if _, err := demands.Expand(); err == nil {
		t.Fatal("expected out-of-range demand path index to fail expansion")
	}
}
