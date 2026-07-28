package typefacts

import (
	"path/filepath"
	"sort"
)

// retainedDemandStore is the session-owned canonical demand set. Its sorted
// groups are mutated transactionally: sparse analyzes edit only named paths,
// while an undo log restores the preceding state if any later stage fails.
type retainedDemandStore struct {
	groups             []DemandGroup
	undoScratch        []demandUndo
	changedPathScratch []string
	runScratch         []DemandGroup
}

type demandUndoKind uint8

const (
	demandUndoReplace demandUndoKind = iota
	demandUndoInsert
	demandUndoRemove
)

type demandUndo struct {
	kind     demandUndoKind
	index    int
	previous DemandGroup
}

type retainedDemandTransaction struct {
	store        *retainedDemandStore
	undo         []demandUndo
	changedPaths []string
	changedRuns  []DemandGroup
	resetOld     []DemandGroup
	reset        bool
	finished     bool
}

func (s *retainedDemandStore) begin(
	changes []EntityDemand,
	removed []string,
	reset bool,
) retainedDemandTransaction {
	changedRuns := sessionChangedDemandRuns(changes, s.runScratch[:0])
	for index := range changedRuns {
		changedRuns[index].Demands = canonicalDemandRun(changedRuns[index].Path, changedRuns[index].Demands)
	}
	s.runScratch = nil
	changedPaths := s.changedPathScratch[:0]
	for _, group := range changedRuns {
		changedPaths = append(changedPaths, filepath.Clean(group.Path))
	}
	for _, path := range removed {
		changedPaths = append(changedPaths, filepath.Clean(path))
	}
	sort.Strings(changedPaths)
	changedPaths = compactSortedPaths(changedPaths)
	s.changedPathScratch = nil

	transaction := retainedDemandTransaction{
		store:        s,
		undo:         s.undoScratch[:0],
		changedPaths: changedPaths,
		changedRuns:  changedRuns,
		reset:        reset,
	}
	s.undoScratch = nil
	if reset {
		transaction.resetOld = s.groups
		s.groups = make([]DemandGroup, 0, len(changedRuns))
	}
	for _, group := range changedRuns {
		transaction.replace(group)
	}
	for _, path := range removed {
		transaction.remove(filepath.Clean(path))
	}
	return transaction
}

func (t *retainedDemandTransaction) groups() []DemandGroup {
	return t.store.groups
}

func (t *retainedDemandTransaction) paths() []string {
	return t.changedPaths
}

func (t *retainedDemandTransaction) replace(group DemandGroup) {
	groups := t.store.groups
	index := sort.Search(len(groups), func(index int) bool {
		return groups[index].Path >= group.Path
	})
	if index < len(groups) && groups[index].Path == group.Path {
		t.undo = append(t.undo, demandUndo{
			kind:     demandUndoReplace,
			index:    index,
			previous: groups[index],
		})
		groups[index] = group
		return
	}
	t.undo = append(t.undo, demandUndo{kind: demandUndoInsert, index: index})
	groups = append(groups, DemandGroup{})
	copy(groups[index+1:], groups[index:])
	groups[index] = group
	t.store.groups = groups
}

func (t *retainedDemandTransaction) remove(path string) {
	groups := t.store.groups
	index := sort.Search(len(groups), func(index int) bool {
		return groups[index].Path >= path
	})
	if index == len(groups) || groups[index].Path != path {
		return
	}
	t.undo = append(t.undo, demandUndo{
		kind:     demandUndoRemove,
		index:    index,
		previous: groups[index],
	})
	copy(groups[index:], groups[index+1:])
	groups[len(groups)-1] = DemandGroup{}
	t.store.groups = groups[:len(groups)-1]
}

func (t *retainedDemandTransaction) commit() {
	t.finish()
}

func (t *retainedDemandTransaction) rollback() {
	if t.finished {
		return
	}
	if t.reset {
		clear(t.store.groups)
		t.store.groups = t.resetOld
		t.finish()
		return
	}
	for index := len(t.undo) - 1; index >= 0; index-- {
		undo := t.undo[index]
		groups := t.store.groups
		switch undo.kind {
		case demandUndoReplace:
			groups[undo.index] = undo.previous
		case demandUndoInsert:
			copy(groups[undo.index:], groups[undo.index+1:])
			groups[len(groups)-1] = DemandGroup{}
			t.store.groups = groups[:len(groups)-1]
		case demandUndoRemove:
			groups = append(groups, DemandGroup{})
			copy(groups[undo.index+1:], groups[undo.index:])
			groups[undo.index] = undo.previous
			t.store.groups = groups
		}
	}
	t.finish()
}

func (t *retainedDemandTransaction) finish() {
	if t.finished {
		return
	}
	t.finished = true
	clear(t.undo)
	t.store.undoScratch = t.undo[:0]
	clear(t.changedPaths)
	t.store.changedPathScratch = t.changedPaths[:0]
	clear(t.changedRuns)
	t.store.runScratch = t.changedRuns[:0]
	t.resetOld = nil
}

func (s *retainedDemandStore) at(path string) []EntityDemand {
	path = filepath.Clean(path)
	index := sort.Search(len(s.groups), func(index int) bool {
		return s.groups[index].Path >= path
	})
	if index == len(s.groups) || s.groups[index].Path != path {
		return nil
	}
	return s.groups[index].Demands
}

// sessionChangedDemandRuns borrows contiguous path-ordered runs directly from
// the decoded request. Non-canonical callers take a small compatibility path
// that groups and path-sorts only the changed subset; begin establishes each
// run's location order before retained ownership.
func sessionChangedDemandRuns(
	changes []EntityDemand,
	scratch []DemandGroup,
) []DemandGroup {
	canonical := true
	for start := 0; start < len(changes); {
		path := filepath.Clean(changes[start].Location.Path)
		end := start + 1
		for end < len(changes) && filepath.Clean(changes[end].Location.Path) == path {
			end++
		}
		if len(scratch) != 0 && scratch[len(scratch)-1].Path >= path {
			canonical = false
			break
		}
		scratch = append(scratch, DemandGroup{
			Path:    path,
			Demands: changes[start:end],
		})
		start = end
	}
	if canonical {
		return scratch
	}

	clear(scratch)
	scratch = scratch[:0]
	grouped := make(map[string][]EntityDemand)
	for _, demand := range changes {
		path := filepath.Clean(demand.Location.Path)
		grouped[path] = append(grouped[path], demand)
	}
	paths := make([]string, 0, len(grouped))
	for path := range grouped {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	for _, path := range paths {
		scratch = append(scratch, DemandGroup{Path: path, Demands: grouped[path]})
	}
	return scratch
}

func compactSortedPaths(paths []string) []string {
	write := 0
	for _, path := range paths {
		if write != 0 && paths[write-1] == path {
			continue
		}
		paths[write] = path
		write++
	}
	clear(paths[write:])
	return paths[:write]
}
