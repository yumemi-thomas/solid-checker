package typefacts

// WireTransitionEncoderForTest exposes the session-owned encoder to external
// benchmarks without widening the production API.
type WireTransitionEncoderForTest struct {
	encoder wireTransitionEncoder
}

func (e *WireTransitionEncoderForTest) Full(projectID string, target *FactTable) ([]byte, error) {
	transition, err := e.encoder.Encode(wireTransitionInput{
		ProjectID: projectID,
		Target:    target,
	})
	return transition.Bytes, err
}

func (e *WireTransitionEncoderForTest) Delta(
	projectID, baseStateToken string,
	base, target *FactTable,
) ([]byte, error) {
	transition, err := e.encoder.Encode(wireTransitionInput{
		ProjectID:      projectID,
		BaseStateToken: baseStateToken,
		Base:           base,
		Target:         target,
	})
	return transition.Bytes, err
}

func DropTransportEvidenceForTest(table *FactTable) {
	table.transport = nil
}

func SymbolFactsForTest(table *FactTable) []SymbolFact {
	facts := make([]SymbolFact, 0, table.symbolFactsCount())
	table.rangeSymbolFacts(func(fact SymbolFact) {
		facts = append(facts, fact)
	})
	return facts
}
