package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

// reactorProject writes a small real project: TypeScript, JavaScript and an
// empty source, so Sources carries a zero-byte file.
func reactorProject(t *testing.T) string {
	t.Helper()
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	for name, contents := range map[string]string{
		"tsconfig.json": `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","allowJs":true,"types":[]},"include":["*.ts","*.js"]}`,
		"main.ts":       "import { other } from \"./other.js\";\nexport const exact = other;\n",
		"other.js":      "export const other = 1;\n",
		"empty.ts":      "",
	} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return filepath.Join(root, "tsconfig.json")
}

func requestFrame(t *testing.T, request typefacts.LifecycleRequest) []byte {
	t.Helper()
	payload, err := wirecbor.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	frame := binary.LittleEndian.AppendUint32(nil, uint32(len(payload)))
	return append(frame, payload...)
}

func lifecycleRequests(project string) []typefacts.LifecycleRequest {
	base := func(id uint64, operation typefacts.LifecycleOperation) typefacts.LifecycleRequest {
		return typefacts.LifecycleRequest{
			Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: id, Operation: operation,
			ProjectID: project, Generation: 1,
		}
	}
	modules := base(3, typefacts.LifecycleModules)
	modules.ModuleGraph = &typefacts.ModuleInventoryDemand{Imports: true, Packages: true}
	stale := base(4, typefacts.LifecycleSources)
	stale.Generation = 2
	return []typefacts.LifecycleRequest{
		base(1, typefacts.LifecycleOpen),
		base(2, typefacts.LifecycleSources),
		modules,
		stale,
	}
}

// TestReactorFramesEqualTheStdioService drives the same requests through the
// stdio service (with inline sources) and the owned reactor: every response
// frame, including a refused stale-generation one, is byte-identical.
func TestReactorFramesEqualTheStdioService(t *testing.T) {
	project := reactorProject(t)
	requests := lifecycleRequests(project)
	var input bytes.Buffer
	for _, request := range requests {
		input.Write(requestFrame(t, request))
	}
	var stdout bytes.Buffer
	if err := run(context.Background(), []string{"-project", project, "-inline-sources"}, &input, &stdout); err != nil {
		t.Fatal(err)
	}
	reader := bufio.NewReader(&stdout)
	readOne := func() []byte {
		t.Helper()
		payload, err := readFrame(reader)
		if err != nil {
			t.Fatal(err)
		}
		return append(binary.LittleEndian.AppendUint32(nil, uint32(len(payload))), payload...)
	}

	reactor := newOwnedReactor()
	handshake, err := reactor.handshakeFrame()
	if err != nil {
		t.Fatal(err)
	}
	if stdio := readOne(); !bytes.Equal(stdio, handshake) {
		t.Fatalf("handshake differs:\n%x\n%x", stdio, handshake)
	}
	envelope, err := wirecbor.Marshal(ownedOpenRequest{ProjectID: project})
	if err != nil {
		t.Fatal(err)
	}
	handle, err := reactor.open(envelope)
	if err != nil || handle != 1 {
		t.Fatalf("open = %d, %v", handle, err)
	}
	for _, request := range requests {
		owned, err := reactor.lifecycle(handle, requestFrame(t, request))
		if err != nil {
			t.Fatal(err)
		}
		if stdio := readOne(); !bytes.Equal(stdio, owned) {
			t.Fatalf("%s response differs:\n%x\n%x", request.Operation, stdio, owned)
		}
	}
	if _, err := reader.ReadByte(); !errors.Is(err, io.EOF) {
		t.Fatalf("stdio service wrote more frames than requests: %v", err)
	}
}

// TestInlineSourcesEqualTheArena compares the two Sources carriers of one
// project: the same paths, the same exact bytes (an empty file included) and
// no local flag.
func TestInlineSourcesEqualTheArena(t *testing.T) {
	project := reactorProject(t)
	request := lifecycleRequests(project)[1]
	answer := func(inline bool) typefacts.LifecycleResponse {
		reactor := newOwnedReactor()
		envelope, err := wirecbor.Marshal(ownedOpenRequest{ProjectID: project})
		if err != nil {
			t.Fatal(err)
		}
		handle, err := reactor.open(envelope)
		if err != nil {
			t.Fatal(err)
		}
		defer reactor.close(handle)
		program, err := reactor.program(handle)
		if err != nil {
			t.Fatal(err)
		}
		if !inline {
			session, err := typefacts.NewSessionWithOptions(mustOpen(t, project), project, nil, typefacts.SessionOptions{BuildID: buildID})
			if err != nil {
				t.Fatal(err)
			}
			defer session.Close()
			program = &ownedProgram{session: session, responder: &lifecycleResponder{session: session}}
		}
		open := lifecycleRequests(project)[0]
		if response := program.session.Lifecycle(context.Background(), open); !response.OK {
			t.Fatalf("open: %+v", response.Error)
		}
		response := program.session.Lifecycle(context.Background(), request)
		if !response.OK {
			t.Fatalf("sources: %+v", response.Error)
		}
		if inline && (response.SourceArena != "" || len(response.SourceLengths) != 0) {
			t.Fatalf("inline sources also carried an arena: %q %v", response.SourceArena, response.SourceLengths)
		}
		if !inline {
			arena, err := os.ReadFile(response.SourceArena)
			if err != nil {
				t.Fatal(err)
			}
			offset := uint64(0)
			for index := range response.Sources {
				length := response.SourceLengths[index]
				response.Sources[index].Source = arena[offset : offset+length]
				offset += length
			}
			if offset != uint64(len(arena)) {
				t.Fatalf("arena holds %d bytes, descriptors %d", len(arena), offset)
			}
		}
		return response
	}
	inline, arena := answer(true), answer(false)
	if len(inline.Sources) != 3 || len(inline.Sources) != len(arena.Sources) {
		t.Fatalf("source counts %d, %d", len(inline.Sources), len(arena.Sources))
	}
	empty := false
	for index := range inline.Sources {
		left, right := inline.Sources[index], arena.Sources[index]
		if left.Path != right.Path || !bytes.Equal(left.Source, right.Source) || left.Local || right.Local {
			t.Fatalf("source %d differs: %q vs %q", index, left.Path, right.Path)
		}
		if strings.HasSuffix(left.Path, "empty.ts") {
			empty = len(left.Source) == 0
		}
	}
	if !empty {
		t.Fatal("the empty source is not carried as zero bytes")
	}
}

func mustOpen(t *testing.T, project string) typefacts.Project {
	t.Helper()
	backend, err := tsgo.OpenProject(context.Background(), project, nil)
	if err != nil {
		t.Fatal(err)
	}
	return backend
}

func TestReactorHandleOwnership(t *testing.T) {
	project := reactorProject(t)
	reactor := newOwnedReactor()
	envelope := func(request ownedOpenRequest) []byte {
		encoded, err := wirecbor.Marshal(request)
		if err != nil {
			t.Fatal(err)
		}
		return encoded
	}
	open := requestFrame(t, lifecycleRequests(project)[0])

	if _, err := reactor.open(envelope(ownedOpenRequest{ProjectID: filepath.Join(filepath.Dir(project), "missing", "tsconfig.json")})); err == nil {
		t.Fatal("a project that cannot open was accepted")
	}
	if _, err := reactor.open(envelope(ownedOpenRequest{ProjectID: "relative/tsconfig.json"})); err == nil {
		t.Fatal("a relative project identity was accepted")
	}
	if _, err := reactor.open(envelope(ownedOpenRequest{ProjectID: project, Config: []byte("{}")})); err == nil {
		t.Fatal("configuration bytes without hasConfig were accepted")
	}
	if reactor.next != 0 || len(reactor.programs) != 0 {
		t.Fatalf("failed opens issued handles: next %d, open %d", reactor.next, len(reactor.programs))
	}

	first, err := reactor.open(envelope(ownedOpenRequest{ProjectID: project}))
	if err != nil {
		t.Fatal(err)
	}
	// A configuration overlay at a logical path that does not exist on disk.
	logical := filepath.Join(filepath.Dir(project), "tsconfig.generated.json")
	config, err := os.ReadFile(project)
	if err != nil {
		t.Fatal(err)
	}
	second, err := reactor.open(envelope(ownedOpenRequest{ProjectID: logical, HasConfig: true, Config: config}))
	if err != nil {
		t.Fatal(err)
	}
	if first != 1 || second != 2 {
		t.Fatalf("handles %d, %d", first, second)
	}
	logicalOpen := requestFrame(t, lifecycleRequests(logical)[0])
	for handle, frame := range map[uint32][]byte{first: open, second: logicalOpen} {
		if _, err := reactor.lifecycle(handle, frame); err != nil {
			t.Fatalf("handle %d: %v", handle, err)
		}
	}
	// A request for the other program is answered by this program's session,
	// which refuses the foreign project identity.
	refused, err := reactor.lifecycle(first, logicalOpen)
	if err != nil {
		t.Fatal(err)
	}
	var response typefacts.LifecycleResponse
	if err := wirecbor.Unmarshal(refused[4:], &response); err != nil || response.OK || response.Error == nil {
		t.Fatalf("cross-program request answered: %+v %v", response, err)
	}

	program, _ := reactor.program(second)
	program.busy = true
	if _, err := reactor.lifecycle(second, logicalOpen); !errors.Is(err, errBusyHandle) {
		t.Fatalf("busy lifecycle: %v", err)
	}
	if err := reactor.close(second); !errors.Is(err, errBusyHandle) {
		t.Fatalf("busy close: %v", err)
	}
	program.busy = false

	if err := reactor.close(first); err != nil {
		t.Fatal(err)
	}
	if err := reactor.close(first); err != nil {
		t.Fatalf("second close: %v", err)
	}
	if _, err := reactor.lifecycle(first, open); !errors.Is(err, errClosedHandle) {
		t.Fatalf("closed lifecycle: %v", err)
	}
	for _, foreign := range []uint32{0, 3, 99} {
		if _, err := reactor.lifecycle(foreign, open); !errors.Is(err, errForeignHandle) {
			t.Fatalf("foreign %d lifecycle: %v", foreign, err)
		}
		if err := reactor.close(foreign); !errors.Is(err, errForeignHandle) {
			t.Fatalf("foreign %d close: %v", foreign, err)
		}
	}
	third, err := reactor.open(envelope(ownedOpenRequest{ProjectID: project}))
	if err != nil || third != 3 {
		t.Fatalf("a closed handle was reused: %d %v", third, err)
	}
	_ = reactor.close(second)
	_ = reactor.close(third)
}

func TestFrameBodyRefusesMalformedFrames(t *testing.T) {
	good := append(binary.LittleEndian.AppendUint32(nil, 3), 'a', 'b', 'c')
	if body, err := frameBody(good); err != nil || string(body) != "abc" {
		t.Fatalf("good frame: %q %v", body, err)
	}
	for name, frame := range map[string][]byte{
		"short prefix": {1, 0},
		"truncated":    good[:5],
		"trailing":     append(append([]byte(nil), good...), 'd'),
		"oversized":    binary.LittleEndian.AppendUint32(nil, uint32(wirecbor.MaxMessageBytes+1)),
	} {
		if _, err := frameBody(frame); err == nil {
			t.Errorf("%s frame accepted", name)
		}
	}
}

// TestSharedEncoderKeepsBothSelections checks the one response encoder on a
// small response carrying invocation fields and on a large table transition:
// each decodes to exactly what a plain Marshal of the same value encodes.
func TestSharedEncoderKeepsBothSelections(t *testing.T) {
	small := typefacts.LifecycleResponse{
		Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: 5, ProjectID: "/p/tsconfig.json", Generation: 1, OK: true,
		InvocationEnvelope:  &typefacts.InvocationEnvelope{Generation: 1},
		ExportValueEnvelope: &typefacts.InvocationEnvelope{Generation: 1},
	}
	large := typefacts.LifecycleResponse{
		Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: 6, ProjectID: "/p/tsconfig.json", Generation: 1, OK: true,
		TableTransition: bytes.Repeat([]byte{7}, 1<<20), StateToken: "2",
	}
	refused := typefacts.LifecycleResponse{
		Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: 7, ProjectID: "/p/tsconfig.json", Generation: 1,
		Error: &typefacts.LifecycleError{Code: "generation-mismatch", Message: "stale"},
	}
	for name, value := range map[string]typefacts.LifecycleResponse{"small": small, "large": large, "error": refused} {
		chunks, size, err := encodeLifecycleResponse(value)
		if err != nil {
			t.Fatal(err)
		}
		expected, err := wirecbor.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		frame, err := singleFrame(chunks, size)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(frame[4:], expected) || binary.LittleEndian.Uint32(frame) != uint32(len(expected)) {
			t.Fatalf("%s: shared encoding differs from Marshal", name)
		}
		if (name == "large") != (len(chunks) > 1) {
			t.Fatalf("%s: %d fragments", name, len(chunks))
		}
	}
}
