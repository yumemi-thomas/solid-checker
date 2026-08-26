package typefacts

import "testing"

func TestWireTransitionFrameOwnershipMatchesBufferLifetime(t *testing.T) {
	small := []byte("small")
	smallRows := make([]byte, 4, 16)
	smallEncoder := wireTransitionEncoder{}
	smallOwned := smallEncoder.detachFrame(small, smallRows)
	small[0] = 'X'
	if string(smallOwned) != "small" {
		t.Fatal("small response aliases reusable frame scratch")
	}
	if smallEncoder.frame == nil || smallEncoder.rows == nil {
		t.Fatal("small response discarded reusable scratch")
	}

	large := make([]byte, maxRetainedWireTransitionBuffer)
	largeRows := make([]byte, maxRetainedWireTransitionBuffer)
	largeEncoder := wireTransitionEncoder{
		dict:      newStringTableV3(),
		pathOps:   make([]wireTransitionPathOp, 1),
		symbolOps: make([]wireTransitionSymbolOp, 1),
		paths:     make([]string, 1),
		symbolIDs: make([]SymbolID, 1),
	}
	largeEncoder.dict.intern("cold-only")
	largeOwned := largeEncoder.detachFrame(large, largeRows)
	large[0] = 1
	if largeOwned[0] != 1 {
		t.Fatal("large response was copied instead of transferred")
	}
	if largeEncoder.frame != nil || largeEncoder.rows != nil {
		t.Fatal("large response scratch stayed retained after ownership transfer")
	}
	if largeEncoder.dict != nil {
		t.Fatal("large response dictionary stayed retained after ownership transfer")
	}
	if largeEncoder.pathOps != nil || largeEncoder.symbolOps != nil ||
		largeEncoder.paths != nil || largeEncoder.symbolIDs != nil {
		t.Fatal("large response plan stayed retained after ownership transfer")
	}
}
