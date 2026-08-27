// Command transcript-normalizer compares two captured Type Facts response
// transcripts after zeroing only nondeterministic nanosecond duration fields.
// Semantic counters and flags in the timings object remain part of the proof.
package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"os"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

func main() {
	if len(os.Args) != 3 {
		fmt.Fprintln(os.Stderr, "usage: transcript-normalizer OLD_RESPONSES NEW_RESPONSES")
		os.Exit(2)
	}
	left, leftFrames, err := normalize(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	right, rightFrames, err := normalize(os.Args[2])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Printf("%x  %s (normalized)\n", sha256.Sum256(left), os.Args[1])
	fmt.Printf("%x  %s (normalized)\n", sha256.Sum256(right), os.Args[2])
	if !bytes.Equal(left, right) {
		if prefix := os.Getenv("TYPEFACTS_TRANSCRIPT_DUMP_PREFIX"); prefix != "" {
			writeJSON(prefix+"-left.json", leftFrames)
			writeJSON(prefix+"-right.json", rightFrames)
		}
		fmt.Fprintln(os.Stderr, "normalized response transcripts differ")
		os.Exit(1)
	}
}

func writeJSON(path string, value any) {
	encoded, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		panic(err)
	}
	if err := os.WriteFile(path, encoded, 0o600); err != nil {
		panic(err)
	}
}

func normalize(path string) ([]byte, []any, error) {
	contents, err := os.ReadFile(path)
	if err != nil {
		return nil, nil, fmt.Errorf("read %s: %w", path, err)
	}
	var output bytes.Buffer
	var frames []any
	for frame := 0; len(contents) != 0; frame++ {
		if len(contents) < 4 {
			return nil, nil, fmt.Errorf("%s: truncated frame header", path)
		}
		length := int(binary.LittleEndian.Uint32(contents[:4]))
		contents = contents[4:]
		if length > len(contents) {
			return nil, nil, fmt.Errorf("%s: truncated frame body", path)
		}
		payload := contents[:length]
		contents = contents[length:]
		var normalized []byte
		if frame == 0 {
			var handshake typefacts.ServiceHandshake
			if err := wirecbor.Unmarshal(payload, &handshake); err != nil {
				return nil, nil, fmt.Errorf("%s: decode handshake: %w", path, err)
			}
			frames = append(frames, handshake)
			normalized, err = wirecbor.Marshal(handshake)
		} else {
			var response typefacts.LifecycleResponse
			if err := wirecbor.Unmarshal(payload, &response); err != nil {
				return nil, nil, fmt.Errorf("%s: decode response %d: %w", path, frame, err)
			}
			frames = append(frames, response)
			if response.Timings != nil {
				response.Timings.RequestDecodeNs = 0
				response.Timings.AnalyzeNs = 0
				response.Timings.AsyncNs = 0
				response.Timings.DemandNs = 0
				response.Timings.AssemblyNs = 0
				response.Timings.SortNs = 0
				response.Timings.CloseSymbolsNs = 0
			}
			if response.SourceArena != "" {
				// The absolute temp filename is transport-local. Keep its presence
				// and the exact sourceLengths/content proof, but canonicalize the
				// random path allocated independently by each process.
				response.SourceArena = "<source-arena>"
			}
			normalized, err = wirecbor.Marshal(response)
		}
		if err != nil {
			return nil, nil, fmt.Errorf("%s: encode frame %d: %w", path, frame, err)
		}
		if err := binary.Write(&output, binary.LittleEndian, uint32(len(normalized))); err != nil {
			return nil, nil, err
		}
		output.Write(normalized)
	}
	return output.Bytes(), frames, nil
}
