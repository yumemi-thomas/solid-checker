package typefacts_test

import (
	"os"
	"regexp"
	"testing"
)

func TestAllTypeScriptShimsUseOneReviewedTsgolintRevision(t *testing.T) {
	contents, err := os.ReadFile("../../go.mod")
	if err != nil {
		t.Fatal(err)
	}
	matches := regexp.MustCompile(`github\.com/oxc-project/tsgolint/shim/\S+\s+(v0\.0\.0-\d+-[0-9a-f]+)`).FindAllSubmatch(contents, -1)
	if len(matches) != 9 {
		t.Fatalf("tsgolint shim replacements = %d, want 9", len(matches))
	}
	want := string(matches[0][1])
	for _, match := range matches[1:] {
		if string(match[1]) != want {
			t.Fatalf("mixed tsgolint revisions: %q and %q", want, match[1])
		}
	}
}
