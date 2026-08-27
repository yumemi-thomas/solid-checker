package typefacts

import (
	"fmt"
	"testing"
)

var benchmarkDetachedTransition []byte

func BenchmarkWireTransitionFrameOwnership(b *testing.B) {
	for _, size := range []int{1 << 20, 10 << 20, 50 << 20} {
		b.Run(fmt.Sprintf("%dMiB", size>>20), func(b *testing.B) {
			rows := make([]byte, size)
			encoder := wireTransitionEncoder{frame: make([]byte, 0, size)}
			b.ReportAllocs()
			for b.Loop() {
				encoder.frame = append(encoder.frame[:0], rows...)
				benchmarkDetachedTransition = encoder.detachFrame(encoder.frame, nil)
			}
		})
	}
}
