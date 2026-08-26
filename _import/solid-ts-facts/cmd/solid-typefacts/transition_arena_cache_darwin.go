//go:build darwin

package main

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func disableTransitionArenaCache(file *os.File) error {
	if _, err := unix.FcntlInt(file.Fd(), unix.F_NOCACHE, 1); err != nil {
		return fmt.Errorf("disable transition arena cache: %w", err)
	}
	return nil
}
