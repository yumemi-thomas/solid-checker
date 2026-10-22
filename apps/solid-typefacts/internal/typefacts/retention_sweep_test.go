package typefacts_test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestRetentionSweepOverFixtureProjects runs a cold analysis, a span-shifting
// update and the restoring update on fixture projects, comparing the retained
// session with a fresh open after each. SOLID_RETENTION_SWEEP=<stride> enables
// it (every stride-th project of fixtures/).
func TestRetentionSweepOverFixtureProjects(t *testing.T) {
	strideText := os.Getenv("SOLID_RETENTION_SWEEP")
	if strideText == "" {
		t.Skip("set SOLID_RETENTION_SWEEP=<stride> to sweep the fixture projects")
	}
	stride, err := strconv.Atoi(strideText)
	if err != nil || stride < 1 {
		t.Fatalf("bad SOLID_RETENTION_SWEEP %q", strideText)
	}
	ctx := context.Background()
	repo, err := filepath.Abs("../../../..")
	if err != nil {
		t.Fatal(err)
	}
	var configs []string
	_ = filepath.WalkDir(filepath.Join(repo, "fixtures"), func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if entry.IsDir() && (entry.Name() == "node_modules" || entry.Name() == ".git") {
			return filepath.SkipDir
		}
		if !entry.IsDir() && entry.Name() == "tsconfig.json" {
			configs = append(configs, path)
		}
		return nil
	})
	sort.Strings(configs)
	failures, compared := 0, 0
	for index := 0; index < len(configs); index += stride {
		config := configs[index]
		label := strings.TrimPrefix(config, repo+"/")
		session := openRetentionSession(t, config).closure
		demands := textDemands(t, session, withReferenceSpace)
		if len(demands) == 0 {
			continue
		}
		if _, err := session.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
			t.Logf("%s: cold analysis refused: %v", label, err)
			continue
		}
		sources, _ := session.SourceFiles(ctx)
		var target *typefacts.SourceFile
		for i := range sources {
			if !strings.Contains(sources[i].Path, "/node_modules/") && !strings.HasSuffix(sources[i].Path, ".d.ts") &&
				strings.HasSuffix(sources[i].Path, ".ts") && len(sources[i].Source) < 60000 {
				target = &sources[i]
				break
			}
		}
		if target == nil {
			continue
		}
		changes := []typefacts.FileChange{
			{Path: target.Path, Version: 1, Source: append([]byte("\n"), target.Source...)},
			{Path: target.Path, Version: 2, Source: target.Source},
		}
		for step, change := range changes {
			if _, err := session.Update(ctx, []typefacts.FileChange{change}); err != nil {
				t.Fatalf("%s update %d: %v", label, step, err)
			}
			retainedDemands := textDemands(t, session, withReferenceSpace)
			retained, err := session.DemandTableForGroups(ctx, uint64(step+2), groupedDemands(retainedDemands), nil)
			if err != nil {
				t.Fatalf("%s retained %d: %v", label, step, err)
			}
			fresh := openRetentionSession(t, config).closure
			for replay := 0; replay <= step; replay++ {
				if _, err := fresh.Update(ctx, []typefacts.FileChange{changes[replay]}); err != nil {
					t.Fatalf("%s replay: %v", label, err)
				}
			}
			freshDemands := textDemands(t, fresh, withReferenceSpace)
			whole, err := fresh.DemandTableForGroups(ctx, uint64(step+2), canonicalDemandGroups(freshDemands), nil)
			if err != nil {
				t.Fatalf("%s fresh %d: %v", label, step, err)
			}
			compared++
			left, right := renameCounters(describeEntities(retained)), renameCounters(describeEntities(whole))
			if report := fieldDifferences(left, right); report != "" {
				failures++
				t.Errorf("%s [%s]:\n%s", label, []string{"shifted", "restored"}[step], report)
			}
		}
	}
	t.Logf("compared %d retained/fresh generations, %d differ", compared, failures)
}

// fieldDifferences pairs rows by location and names the top-level fields that
// differ, so a report stays readable.
func fieldDifferences(retained, fresh []string) string {
	rows := func(lines []string) map[string]map[string]json.RawMessage {
		out := make(map[string]map[string]json.RawMessage, len(lines))
		for _, line := range lines {
			var row map[string]json.RawMessage
			if err := json.Unmarshal([]byte(line), &row); err != nil {
				panic(err)
			}
			out[string(row["location"])] = row
		}
		return out
	}
	left, right := rows(retained), rows(fresh)
	var report strings.Builder
	shown := 0
	for key, leftRow := range left {
		rightRow, ok := right[key]
		if !ok {
			continue
		}
		for field, leftValue := range leftRow {
			if string(leftValue) != string(rightRow[field]) && shown < 6 {
				shown++
				fmt.Fprintf(&report, "  %.100s %s: retained %.160s | fresh %.160s\n", key, field, leftValue, rightRow[field])
			}
		}
		for field, rightValue := range rightRow {
			if _, ok := leftRow[field]; !ok && shown < 6 {
				shown++
				fmt.Fprintf(&report, "  %.100s %s: retained <absent> | fresh %.160s\n", key, field, rightValue)
			}
		}
	}
	if report.Len() == 0 && len(left) != len(right) {
		fmt.Fprintf(&report, "  row sets differ (%d vs %d)\n", len(left), len(right))
	}
	return report.String()
}
