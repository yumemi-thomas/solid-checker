//go:build wasip1

package main

import (
	"encoding/binary"
	"unsafe"
)

// The owned reactor's ABI. The host writes one input into the buffer
// allocate_input returns, calls one operation, then copies output_length bytes
// from output_pointer before its next call: the output buffer is replaced by
// every operation and must not be retained across calls or memory growth.
// Every operation returns 0 on success; on failure it returns 1 and the
// output holds a UTF-8 message.
//
//   - typefacts_handshake: output is the startup frame.
//   - typefacts_open: input is the CBOR open envelope; output is the new
//     handle as four little-endian bytes.
//   - typefacts_lifecycle(handle): input is one request frame; output is one
//     response frame.
//   - typefacts_close(handle): releases the program; output is empty.
var (
	reactor = newOwnedReactor()
	input   []byte
	output  []byte
)

//go:wasmexport allocate_input
func allocateInput(size uint32) uint32 {
	input = make([]byte, size)
	if len(input) == 0 {
		return 0
	}
	return uint32(uintptr(unsafe.Pointer(unsafe.SliceData(input))))
}

//go:wasmexport output_pointer
func outputPointer() uint32 {
	if len(output) == 0 {
		return 0
	}
	return uint32(uintptr(unsafe.Pointer(unsafe.SliceData(output))))
}

//go:wasmexport output_length
func outputLength() uint32 {
	return uint32(len(output))
}

func answer(result []byte, err error) uint32 {
	input = nil
	if err != nil {
		output = []byte(err.Error())
		return 1
	}
	output = result
	return 0
}

//go:wasmexport typefacts_handshake
func typefactsHandshake() uint32 {
	return answer(reactor.handshakeFrame())
}

//go:wasmexport typefacts_open
func typefactsOpen() uint32 {
	handle, err := reactor.open(input)
	if err != nil {
		return answer(nil, err)
	}
	return answer(binary.LittleEndian.AppendUint32(nil, handle), nil)
}

//go:wasmexport typefacts_lifecycle
func typefactsLifecycle(handle uint32) uint32 {
	return answer(reactor.lifecycle(handle, input))
}

//go:wasmexport typefacts_close
func typefactsClose(handle uint32) uint32 {
	return answer(nil, reactor.close(handle))
}
