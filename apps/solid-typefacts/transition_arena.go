package main

import (
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"os"
)

const (
	transitionArenaRowsOffset = 64 << 20
	transitionArenaHeaderSize = 32
)

var transitionArenaMagic = [8]byte{'S', 'T', 'F', 'A', 'R', 'E', 'N', 'A'}

// transitionFileArena is the producer adapter for Rust-owned transition
// storage. Rows start at a fixed sparse-file offset, allowing the dictionary
// prefix to be written immediately before them without shifting or copying the
// completed row run.
type transitionFileArena struct {
	path      string
	file      *os.File
	requestID uint64
	rowBytes  uint64
	offset    uint64
	length    uint64
	committed bool
}

func openTransitionFileArena(path string) (*transitionFileArena, error) {
	if path == "" {
		return nil, nil
	}
	file, err := os.OpenFile(path, os.O_RDWR, 0)
	if err != nil {
		return nil, fmt.Errorf("open transition arena: %w", err)
	}
	if err := disableTransitionArenaCache(file); err != nil {
		_ = file.Close()
		return nil, err
	}
	return &transitionFileArena{path: path, file: file}, nil
}

func (a *transitionFileArena) Close() error {
	if a == nil || a.file == nil {
		return nil
	}
	return a.file.Close()
}

func (a *transitionFileArena) Begin(requestID uint64) error {
	if a == nil || a.file == nil {
		return errors.New("transition arena is closed")
	}
	a.requestID = requestID
	a.rowBytes = 0
	a.offset = 0
	a.length = 0
	a.committed = false
	if err := a.file.Truncate(transitionArenaRowsOffset); err != nil {
		return fmt.Errorf("reset transition arena: %w", err)
	}
	if _, err := a.file.Seek(transitionArenaRowsOffset, io.SeekStart); err != nil {
		return fmt.Errorf("seek transition arena rows: %w", err)
	}
	var empty [transitionArenaHeaderSize]byte
	if _, err := a.file.WriteAt(empty[:], 0); err != nil {
		return fmt.Errorf("clear transition arena header: %w", err)
	}
	return nil
}

func (a *transitionFileArena) AppendRows(chunk []byte) error {
	for len(chunk) != 0 {
		written, err := a.file.Write(chunk)
		if err != nil {
			return err
		}
		if written == 0 {
			return io.ErrShortWrite
		}
		a.rowBytes += uint64(written)
		chunk = chunk[written:]
	}
	return nil
}

func (a *transitionFileArena) Finish(prefix []byte) error {
	if len(prefix) == 0 || len(prefix) > transitionArenaRowsOffset-transitionArenaHeaderSize {
		return fmt.Errorf("transition prefix length %d is out of range", len(prefix))
	}
	offset := uint64(transitionArenaRowsOffset - len(prefix))
	if _, err := a.file.WriteAt(prefix, int64(offset)); err != nil {
		return fmt.Errorf("write transition arena prefix: %w", err)
	}
	length := uint64(len(prefix)) + a.rowBytes
	var header [transitionArenaHeaderSize]byte
	copy(header[:8], transitionArenaMagic[:])
	binary.LittleEndian.PutUint64(header[8:16], a.requestID)
	binary.LittleEndian.PutUint64(header[16:24], offset)
	binary.LittleEndian.PutUint64(header[24:32], length)
	if _, err := a.file.WriteAt(header[:], 0); err != nil {
		return fmt.Errorf("commit transition arena: %w", err)
	}
	a.offset = offset
	a.length = length
	a.committed = true
	return nil
}

func (a *transitionFileArena) descriptor() (string, []uint64, bool) {
	if a == nil || !a.committed {
		return "", nil, false
	}
	return a.path, []uint64{a.requestID, a.offset, a.length}, true
}
