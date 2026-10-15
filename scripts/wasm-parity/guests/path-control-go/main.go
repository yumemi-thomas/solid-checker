// Command path-control-go is a wasip1 control program for the WASM parity
// gate (S3a): Go os rows, recorded as Go's own guest-side path handling
// (syscall/fs_wasip1.go cleans "." and ".." lexically before any WASI call),
// and os.Readlink's growing-buffer retry. Each argument is `id|op|path`; each
// row prints `id|result|detail`.
package main

import (
	"errors"
	"fmt"
	"os"
	"strings"
	"syscall"
)

func outcome(err error) string {
	var errno syscall.Errno
	if errors.As(err, &errno) {
		return fmt.Sprintf("%d", uintptr(errno))
	}
	return "error:" + err.Error()
}

func main() {
	for _, argument := range os.Args[1:] {
		fields := strings.SplitN(argument, "|", 3)
		if len(fields) != 3 {
			fmt.Printf("%s|malformed|\n", argument)
			continue
		}
		id, op, path := fields[0], fields[1], fields[2]
		switch op {
		case "go-stat", "go-lstat":
			stat := os.Stat
			if op == "go-lstat" {
				stat = os.Lstat
			}
			info, err := stat(path)
			if err != nil {
				fmt.Printf("%s|%s|\n", id, outcome(err))
				continue
			}
			fmt.Printf("%s|0|dir=%t link=%t\n", id, info.IsDir(), info.Mode()&os.ModeSymlink != 0)
		case "go-readlink":
			target, err := os.Readlink(path)
			if err != nil {
				fmt.Printf("%s|%s|\n", id, outcome(err))
				continue
			}
			fmt.Printf("%s|0|len=%d\n", id, len(target))
		default:
			fmt.Printf("%s|unknown|%s\n", id, op)
		}
	}
}
