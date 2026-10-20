//go:build !wasip1

package main

import (
	"context"
	"fmt"
	"os"
)

// main serves one project over the stdio lifecycle protocol. The wasip1 build
// is the owned reactor instead (reactor_wasip1.go).
func main() {
	if err := run(context.Background(), os.Args[1:], os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, "solid-typefacts:", err)
		os.Exit(1)
	}
}
