package typefacts

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"testing"
)

type transitionEnvelopeForTest struct {
	version, mode, schema     uint64
	base, target              uint64
	projectID, baseStateToken string
	pathOperations            uint64
	symbolOperations          uint64
}

func decodeTransitionEnvelopeForTest(t *testing.T, frame []byte) transitionEnvelopeForTest {
	t.Helper()
	read := func() uint64 {
		t.Helper()
		value, width := binary.Uvarint(frame)
		if width <= 0 {
			t.Fatal("transition contains a truncated integer")
		}
		frame = frame[width:]
		return value
	}
	envelope := transitionEnvelopeForTest{
		version: read(),
		mode:    read(),
		schema:  read(),
		base:    read(),
		target:  read(),
	}
	dictionary := make([]string, read())
	previous := ""
	for index := range dictionary {
		switch tag := read(); tag {
		case 0:
			prefix, suffixLength := read(), read()
			if prefix > uint64(len(previous)) || suffixLength > uint64(len(frame)) {
				t.Fatal("transition dictionary entry is out of bounds")
			}
			value := previous[:prefix] + string(frame[:suffixLength])
			frame = frame[suffixLength:]
			dictionary[index] = value
			previous = value
		case 1:
			if len(frame) < 12 {
				t.Fatal("transition hashed symbol is truncated")
			}
			dictionary[index] = "symbol:h:" + hex.EncodeToString(frame[:12])
			frame = frame[12:]
		default:
			t.Fatalf("transition dictionary tag = %d", tag)
		}
	}
	lookup := func(label string) string {
		t.Helper()
		index := read()
		if index >= uint64(len(dictionary)) {
			t.Fatalf("%s dictionary index = %d", label, index)
		}
		return dictionary[index]
	}
	envelope.projectID = lookup("project")
	envelope.baseStateToken = lookup("base token")
	envelope.pathOperations = read()
	if envelope.pathOperations == 0 {
		envelope.symbolOperations = read()
	}
	return envelope
}

func TestWireTransitionCarriesIdentityAndDistinguishesReuseFromAdvancement(t *testing.T) {
	table := goldenWireTable()
	encoder := &wireTransitionEncoder{}

	full, err := encoder.Encode(wireTransitionInput{
		ProjectID: table.ProjectID,
		Target:    &table,
	})
	if err != nil {
		t.Fatal(err)
	}
	fullAgain, err := encoder.Encode(wireTransitionInput{
		ProjectID: table.ProjectID,
		Target:    &table,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(full.Bytes, fullAgain.Bytes) {
		t.Fatal("full transition is not deterministic or detached")
	}
	envelope := decodeTransitionEnvelopeForTest(t, full.Bytes)
	if envelope.version != wireTransitionVersion ||
		envelope.mode != uint64(wireTransitionFull) ||
		envelope.schema != TypeFactsTableSchemaVersion ||
		envelope.base != 0 ||
		envelope.target != table.Generation ||
		envelope.projectID != table.ProjectID ||
		envelope.baseStateToken != "" {
		t.Fatalf("full transition identity = %+v", envelope)
	}

	reuse, err := encoder.Encode(wireTransitionInput{
		ProjectID:      table.ProjectID,
		BaseStateToken: "state-3",
		Base:           &table,
		Target:         &table,
	})
	if err != nil {
		t.Fatal(err)
	}
	if reuse.Bytes != nil || reuse.PathOperations != 0 || reuse.SymbolOperations != 0 {
		t.Fatalf("same-generation reuse = %+v", reuse)
	}

	advanced := table
	advanced.Generation++
	delta, err := encoder.Encode(wireTransitionInput{
		ProjectID:      table.ProjectID,
		BaseStateToken: "state-3",
		Base:           &table,
		Target:         &advanced,
	})
	if err != nil {
		t.Fatal(err)
	}
	envelope = decodeTransitionEnvelopeForTest(t, delta.Bytes)
	if envelope.mode != uint64(wireTransitionDelta) ||
		envelope.base != table.Generation ||
		envelope.target != advanced.Generation ||
		envelope.projectID != table.ProjectID ||
		envelope.baseStateToken != "state-3" ||
		envelope.pathOperations != 0 ||
		envelope.symbolOperations != 0 {
		t.Fatalf("generation-advancing empty delta = %+v", envelope)
	}
}

func TestWireTransitionRejectsUnknownEnumsAndRecoversItsScratch(t *testing.T) {
	valid := goldenWireTable()
	invalid := valid
	invalid.Entities = append([]EntityFact(nil), valid.Entities...)
	invalid.Entities[0].Callability = Callability("future")

	encoder := &wireTransitionEncoder{}
	if _, err := encoder.Encode(wireTransitionInput{
		ProjectID: valid.ProjectID,
		Target:    &invalid,
	}); err == nil {
		t.Fatal("unknown callability was encoded")
	}
	recovered, err := encoder.Encode(wireTransitionInput{
		ProjectID: valid.ProjectID,
		Target:    &valid,
	})
	if err != nil {
		t.Fatalf("encode after failure: %v", err)
	}
	fresh, err := (&wireTransitionEncoder{}).Encode(wireTransitionInput{
		ProjectID: valid.ProjectID,
		Target:    &valid,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(recovered.Bytes, fresh.Bytes) {
		t.Fatal("failed encode contaminated reusable scratch")
	}
}
