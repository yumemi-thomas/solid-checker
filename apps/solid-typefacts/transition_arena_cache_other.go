//go:build !darwin

package main

import "os"

func disableTransitionArenaCache(_ *os.File) error {
	return nil
}
