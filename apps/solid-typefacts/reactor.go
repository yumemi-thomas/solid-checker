package main

import (
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"math"
	"path/filepath"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

// ownedReactor is the process-free form of this producer: the same Session,
// request decoder and response encoder as the stdio service, driven one
// synchronous frame at a time by a host that owns the call. It holds several
// programs (an original and a combined program are separate projects) under
// owner-issued handles that are never reused within one reactor lifetime.
//
// It is not an authority: a handle names a program this reactor opened from
// the files its host supplied, nothing more.
type ownedReactor struct {
	next     uint32
	programs map[uint32]*ownedProgram
}

type ownedProgram struct {
	session   *typefacts.Session
	responder *lifecycleResponder
	busy      bool
}

// ownedOpenRequest is the open envelope, deterministic CBOR. Absent and
// empty configuration differ: HasConfig with an empty Config is an overlay of
// zero bytes at ProjectID, not the project's own file.
type ownedOpenRequest struct {
	ProjectID string `cbor:"projectId"`
	HasConfig bool   `cbor:"hasConfig,omitempty"`
	Config    []byte `cbor:"config,omitempty"`
}

var (
	errForeignHandle = errors.New("handle was never issued by this reactor")
	errClosedHandle  = errors.New("handle is closed")
	errBusyHandle    = errors.New("handle already has an exchange in flight")
)

func newOwnedReactor() *ownedReactor {
	return &ownedReactor{programs: make(map[uint32]*ownedProgram)}
}

// handshakeFrame is the startup frame the stdio service writes first,
// length prefix included.
func (r *ownedReactor) handshakeFrame() ([]byte, error) {
	payload, err := handshakePayload()
	if err != nil {
		return nil, err
	}
	return singleFrame([][]byte{payload}, len(payload))
}

// open builds one program from the envelope and returns its new handle. On
// any failure nothing stays open and no handle is issued.
func (r *ownedReactor) open(envelope []byte) (uint32, error) {
	var request ownedOpenRequest
	if err := wirecbor.Unmarshal(envelope, &request); err != nil {
		return 0, fmt.Errorf("decode open request: %w", err)
	}
	if request.ProjectID == "" || !filepath.IsAbs(request.ProjectID) ||
		filepath.Clean(request.ProjectID) != request.ProjectID {
		return 0, fmt.Errorf("project identity %q is not a clean absolute path", request.ProjectID)
	}
	if !request.HasConfig && len(request.Config) != 0 {
		return 0, errors.New("open request carries configuration bytes without hasConfig")
	}
	if r.next == math.MaxUint32 {
		return 0, errors.New("reactor handle space is exhausted")
	}
	var configSource *string
	if request.HasConfig {
		source := string(request.Config)
		configSource = &source
	}
	backend, err := tsgo.OpenProjectWithConfig(context.Background(), request.ProjectID, configSource, nil)
	if err != nil {
		return 0, fmt.Errorf("open TS-Go project: %w", err)
	}
	// NewSessionWithOptions owns backend from here, including on failure.
	session, err := typefacts.NewSessionWithOptions(backend, request.ProjectID, nil, typefacts.SessionOptions{
		BuildID:       buildID,
		InlineSources: true,
	})
	if err != nil {
		return 0, err
	}
	r.next++
	handle := r.next
	r.programs[handle] = &ownedProgram{
		session:   session,
		responder: &lifecycleResponder{session: session},
	}
	return handle, nil
}

// lifecycle answers exactly one request frame with exactly one response
// frame. The returned bytes are a fresh copy owned by the caller.
func (r *ownedReactor) lifecycle(handle uint32, frame []byte) ([]byte, error) {
	program, err := r.program(handle)
	if err != nil {
		return nil, err
	}
	if program.busy {
		return nil, errBusyHandle
	}
	payload, err := frameBody(frame)
	if err != nil {
		return nil, err
	}
	request, err := decodeLifecycleRequest(payload)
	if err != nil {
		return nil, fmt.Errorf("decode request: %w", err)
	}
	program.busy = true
	defer func() { program.busy = false }()
	response := program.responder.lifecycle(context.Background(), request)
	chunks, size, err := encodeLifecycleResponse(response)
	if err != nil {
		return nil, err
	}
	return singleFrame(chunks, size)
}

// close releases a program. Closing an issued handle twice is a no-op; a
// handle this reactor never issued is refused.
func (r *ownedReactor) close(handle uint32) error {
	if handle == 0 || handle > r.next {
		return errForeignHandle
	}
	program, ok := r.programs[handle]
	if !ok {
		return nil
	}
	if program.busy {
		return errBusyHandle
	}
	delete(r.programs, handle)
	return program.session.Close()
}

func (r *ownedReactor) program(handle uint32) (*ownedProgram, error) {
	if handle == 0 || handle > r.next {
		return nil, errForeignHandle
	}
	program, ok := r.programs[handle]
	if !ok {
		return nil, errClosedHandle
	}
	return program, nil
}

// frameBody returns the payload of exactly one u32-little-endian
// length-prefixed frame, refusing a short, oversized or trailing frame.
func frameBody(frame []byte) ([]byte, error) {
	if len(frame) < 4 {
		return nil, errors.New("request frame is shorter than its length prefix")
	}
	size := binary.LittleEndian.Uint32(frame[:4])
	if size > wirecbor.MaxMessageBytes {
		return nil, fmt.Errorf("request is %d bytes, limit is %d", size, wirecbor.MaxMessageBytes)
	}
	if uint64(len(frame)-4) != uint64(size) {
		return nil, fmt.Errorf("request frame holds %d payload bytes, its prefix says %d", len(frame)-4, size)
	}
	return frame[4:], nil
}

// singleFrame joins one payload's fragments behind its length prefix into a
// single owned buffer.
func singleFrame(chunks [][]byte, size int) ([]byte, error) {
	if size > wirecbor.MaxMessageBytes {
		return nil, fmt.Errorf("response is %d bytes, limit is %d", size, wirecbor.MaxMessageBytes)
	}
	frame := make([]byte, 4, 4+size)
	binary.LittleEndian.PutUint32(frame, uint32(size))
	for _, chunk := range chunks {
		frame = append(frame, chunk...)
	}
	if len(frame) != 4+size {
		return nil, fmt.Errorf("response fragments hold %d bytes, not %d", len(frame)-4, size)
	}
	return frame, nil
}
