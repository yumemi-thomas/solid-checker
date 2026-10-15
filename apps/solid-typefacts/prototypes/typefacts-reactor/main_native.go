//go:build !wasip1

package main

import (
	"io"
	"os"
)

// main answers one request read from stdin with exactly the bytes the reactor
// export would leave in its output buffer, followed by one framing newline.
// The exit status is the export's status.
func main() {
	encoded, err := io.ReadAll(os.Stdin)
	var output []byte
	var status uint32
	if err != nil {
		output, status = encode(nil, nil, err)
	} else {
		output, status = respond(encoded)
	}
	if _, err := os.Stdout.Write(append(output, '\n')); err != nil {
		os.Exit(2)
	}
	os.Exit(int(status))
}
