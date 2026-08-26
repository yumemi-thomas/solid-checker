package typefacts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// go:linkname bypasses the type checker: a shim declaration whose signature
// drifts from its target is undefined behavior at runtime, not a compile
// error. This test is the review a compiler bump consists of — it extracts
// every linkname target's definition from the pinned typescript-go source and
// requires the shim declaration to match it parameter for parameter.

var linknameDirective = regexp.MustCompile(
	`^//go:linkname (\S+) github\.com/microsoft/typescript-go/internal/([a-z/]+)\.(.+)$`)

var methodTarget = regexp.MustCompile(`^\(\*(\w+)\)\.(\w+)$`)

func TestLinknameSignaturesMatchThePinnedCompiler(t *testing.T) {
	output, err := exec.Command("go", "list", "-m", "-f", "{{.Dir}}",
		"github.com/microsoft/typescript-go").Output()
	if err != nil {
		t.Fatalf("locate the pinned typescript-go source: %v", err)
	}
	compilerDir := strings.TrimSpace(string(output))

	shimFiles, err := filepath.Glob("../../../../shims/*/shim.go")
	if err != nil {
		t.Fatal(err)
	}
	nested, err := filepath.Glob("../../../../shims/*/*/shim.go")
	if err != nil {
		t.Fatal(err)
	}
	shimFiles = append(shimFiles, nested...)
	if len(shimFiles) != 9 {
		t.Fatalf("shim files = %d, want 9", len(shimFiles))
	}

	verified := 0
	for _, shimFile := range shimFiles {
		contents, err := os.ReadFile(shimFile)
		if err != nil {
			t.Fatal(err)
		}
		lines := strings.Split(string(contents), "\n")
		for index, line := range lines {
			match := linknameDirective.FindStringSubmatch(line)
			if match == nil {
				continue
			}
			name, pkg, target := match[1], match[2], match[3]
			if index+1 >= len(lines) {
				t.Fatalf("%s: linkname %s has no declaration", shimFile, name)
			}
			declaration := lines[index+1]
			targetSignature, receiver := findTargetSignature(t, filepath.Join(compilerDir, "internal", pkg), target)
			if targetSignature == "" {
				t.Errorf("%s: linkname target %s.%s not found in the pinned compiler", shimFile, pkg, target)
				continue
			}
			want := normalizeSignature(targetSignature, pkg, receiver, "")
			got := normalizeSignature(declaration, pkg, "", name)
			if want != got {
				t.Errorf("%s: %s signature drifted\n  compiler: %s\n  shim:     %s", shimFile, name, want, got)
				continue
			}
			verified++
		}
	}
	if verified < 40 {
		t.Fatalf("verified only %d linknames; the sweep is not finding them", verified)
	}
}

// findTargetSignature locates `Func` or `(*Type).method` in the package's
// sources and returns its full signature (joined across wrapped lines, body
// excluded) plus the method receiver name to strip.
func findTargetSignature(t *testing.T, packageDir, target string) (signature, receiver string) {
	t.Helper()
	var prefix *regexp.Regexp
	simpleName := target
	if match := methodTarget.FindStringSubmatch(target); match != nil {
		simpleName = match[2]
		prefix = regexp.MustCompile(`^func \((\w+) \*` + regexp.QuoteMeta(match[1]) + `\) ` + regexp.QuoteMeta(match[2]) + `\(`)
	} else {
		prefix = regexp.MustCompile(`^func ` + regexp.QuoteMeta(target) + `\(`)
	}
	_ = simpleName
	sources, err := filepath.Glob(filepath.Join(packageDir, "*.go"))
	if err != nil {
		t.Fatal(err)
	}
	for _, source := range sources {
		if strings.HasSuffix(source, "_test.go") {
			continue
		}
		contents, err := os.ReadFile(source)
		if err != nil {
			t.Fatal(err)
		}
		lines := strings.Split(string(contents), "\n")
		for index, line := range lines {
			match := prefix.FindStringSubmatch(line)
			if match == nil {
				continue
			}
			// Join wrapped signature lines until the body opens.
			joined := line
			for next := index + 1; !strings.HasSuffix(strings.TrimSpace(joined), "{") && next < len(lines); next++ {
				joined += " " + strings.TrimSpace(lines[next])
			}
			if len(match) > 1 {
				receiver = match[1]
			}
			return joined, receiver
		}
	}
	return "", ""
}

// normalizeSignature reduces a func line to comparable text: the func keyword,
// receiver, name, body brace, whitespace runs, and the target package's own
// qualifier are all removed. For shim declarations, the synthetic leading
// receiver parameter is dropped the same way the compiler side drops its
// receiver clause.
func normalizeSignature(signature, pkg, receiver, shimName string) string {
	s := strings.TrimSuffix(strings.TrimSpace(signature), "{")
	s = regexp.MustCompile(`\s+`).ReplaceAllString(s, " ")
	s = strings.TrimPrefix(s, "func ")
	if receiver != "" {
		s = regexp.MustCompile(`^\(`+regexp.QuoteMeta(receiver)+` \*\w+\) `).ReplaceAllString(s, "")
	}
	s = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*\(`).ReplaceAllString(s, "(")
	if shimName != "" {
		// The shim spells a method as a function whose first parameter is the
		// receiver.
		s = regexp.MustCompile(`^\(recv \*[\w.]+(, |\))`).ReplaceAllStringFunc(s, func(m string) string {
			if strings.HasSuffix(m, ")") {
				return "()"
			}
			return "("
		})
	}
	base := pkg[strings.LastIndex(pkg, "/")+1:]
	s = strings.ReplaceAll(s, base+".", "")
	// Wrapped compiler signatures join with a space after the open paren and
	// a trailing comma before the close; canonicalize both away.
	s = strings.ReplaceAll(s, "( ", "(")
	s = strings.ReplaceAll(s, ", )", ")")
	return strings.TrimSpace(s)
}
