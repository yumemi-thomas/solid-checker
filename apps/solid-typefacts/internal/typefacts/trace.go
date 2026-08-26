package typefacts

import "time"

// Trace receives producer-side observability events.
//
// A nil Trace means tracing is off, and every call site guards on nil *before*
// building an event's payload. That ordering is load-bearing, not stylistic:
// several payloads are O(entities) or O(references) walks, so computing them
// unconditionally and discarding them in a no-op sink would put a whole extra
// pass over the fact table on the latency path. There is deliberately no
// no-op adapter for that reason.
//
// Resolving whether to trace is the adapter's job, done once at construction.
// Nothing below this seam reads the environment or writes to stderr.
type Trace interface {
	// Stage reports one named stage duration.
	Stage(name string, elapsed time.Duration)
	// Metrics reports a named group of counters.
	Metrics(name string, values ...Metric)
}

// Metric is one named counter in a Metrics event.
type Metric struct {
	Key   string
	Value int64
}

// Count builds a Metric from any integer-like value.
func Count[T ~int | ~int64 | ~uint64](key string, value T) Metric {
	return Metric{Key: key, Value: int64(value)}
}

// Flag builds a Metric from a boolean, reported as 0 or 1.
func Flag(key string, value bool) Metric {
	if value {
		return Metric{Key: key, Value: 1}
	}
	return Metric{Key: key, Value: 0}
}

// Nanos builds a Metric from a duration, reported in nanoseconds.
func Nanos(key string, value time.Duration) Metric {
	return Metric{Key: key, Value: value.Nanoseconds()}
}
