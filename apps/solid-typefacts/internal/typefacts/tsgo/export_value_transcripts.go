package tsgo

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strconv"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

var _ typefacts.ExportValueAnalyzer = (*project)(nil)

func (p *project) ExportValueTranscripts(
	ctx context.Context,
	demands []typefacts.ExportValueDemand,
) (typefacts.ExportValueAnswer, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.ExportValueAnswer{}, err
	}
	for _, demand := range demands {
		if demand.CallableDepth < 0 || demand.CallableDepth > typefacts.MaxInvocationCallableDepth {
			return typefacts.ExportValueAnswer{}, fmt.Errorf(
				"export-value callable depth %d exceeds limit %d",
				demand.CallableDepth,
				typefacts.MaxInvocationCallableDepth,
			)
		}
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.ExportValueAnswer{}, ErrClosed
	}

	answer := typefacts.ExportValueAnswer{
		Transcripts: make([]typefacts.ExportValueTranscript, len(demands)),
		Envelope: typefacts.InvocationEnvelope{
			Generation:   p.generation,
			DemandSHA256: exportValueDemandDigest(demands),
		},
	}
	for index, demand := range demands {
		if err := ctx.Err(); err != nil {
			return typefacts.ExportValueAnswer{}, err
		}
		answer.Transcripts[index] = p.exportValueTranscriptLocked(demand)
	}

	inventory, err := p.moduleGraphLocked(ctx, typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		return typefacts.ExportValueAnswer{}, err
	}
	encodedGraph, err := wirecbor.Marshal(inventory)
	if err != nil {
		return typefacts.ExportValueAnswer{}, fmt.Errorf("encode export-value module graph: %w", err)
	}
	answer.Envelope.ModuleGraphSHA256 = sha256String(encodedGraph)
	answer.Envelope.Sources = p.invocationSourceDigestsLocked()
	for _, unresolved := range inventory.Imports {
		if unresolved.ResolvedPath == "" {
			answer.Envelope.OpenReasons = append(answer.Envelope.OpenReasons, "unresolvedModule")
			break
		}
	}
	return answer, nil
}

func (p *project) exportValueTranscriptLocked(
	demand typefacts.ExportValueDemand,
) typefacts.ExportValueTranscript {
	// Keep every early-refusal transcript wire-valid. Callability is a closed
	// string enum on the ordinary CBOR protocol, so its Go zero value is not a
	// serializable verdict. Selection/identity failures still carry an explicit
	// unknown value domain; the outer open reason says why no value was acquired.
	transcript := typefacts.ExportValueTranscript{
		Location: demand.Location,
		Value: typefacts.InvocationValueFact{
			Callability:      typefacts.CallabilityUnknown,
			Constructability: typefacts.InvocationConstructUnknown,
			Primitive:        typefacts.ValuePrimitiveDomain{Unknown: true},
			OpenReasons:      []string{"valueUnavailable"},
		},
	}
	sourceFile, err := p.sourceFileFor(demand.Location)
	if err != nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "sourceUnavailable")
		return transcript
	}
	cursor := semanticNodeCursor{sourceFile: sourceFile}
	node := cursor.exactExpressionAt(demand.Location.StartByte, demand.Location.EndByte)
	if node == nil || !ast.IsIdentifier(node) {
		transcript.OpenReasons = append(transcript.OpenReasons, "identifierNotExact")
		return transcript
	}
	transcript.QueryName = node.Text()
	alias := p.checker.GetSymbolAtLocation(node)
	if alias == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "symbolUnresolved")
		return transcript
	}
	target := p.canonicalSymbol(alias)
	if target == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "aliasUnresolved")
		return transcript
	}
	transcript.Target = p.idFor(target)
	declaration := target.ValueDeclaration
	if declaration == nil && len(target.Declarations) != 0 {
		declaration = target.Declarations[0]
	}
	if declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	transcript.Declaration = p.resolvedDeclaration(nil, declaration, target)
	if transcript.Declaration == nil {
		transcript.OpenReasons = append(transcript.OpenReasons, "declarationUnavailable")
		return transcript
	}
	valueType := p.checker.GetTypeAtLocation(node)
	transcript.Value = p.invocationValueFactLocked(valueType)
	transcript.CallablePaths = p.callablePathsLocked(valueType, demand.CallableDepth)
	transcript.Complete = true
	return transcript
}

func exportValueDemandDigest(demands []typefacts.ExportValueDemand) string {
	hash := sha256.New()
	hashField(hash, "solid-checker:typefacts:export-values:v1")
	for _, demand := range demands {
		hashField(hash, demand.Location.Path)
		hashField(hash, strconv.Itoa(demand.Location.StartByte))
		hashField(hash, strconv.Itoa(demand.Location.EndByte))
		hashField(hash, strconv.Itoa(demand.CallableDepth))
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}
