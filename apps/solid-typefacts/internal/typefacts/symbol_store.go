package typefacts

import "sort"

const symbolFactChunkSize = 256

// symbolFactStore is the immutable canonical symbol representation retained
// between Type Facts session generations. Chunks not touched by an exact
// symbol delta are shared; changed chunks are copied before patching.
type symbolFactStore struct {
	chunks [][]SymbolFact
	length int
}

func newSymbolFactStore(facts []SymbolFact) *symbolFactStore {
	if len(facts) == 0 {
		return &symbolFactStore{}
	}
	store := &symbolFactStore{
		chunks: make([][]SymbolFact, 0, (len(facts)+symbolFactChunkSize-1)/symbolFactChunkSize),
		length: len(facts),
	}
	for start := 0; start < len(facts); start += symbolFactChunkSize {
		end := min(start+symbolFactChunkSize, len(facts))
		store.chunks = append(store.chunks, facts[start:end])
	}
	return store
}

func (s *symbolFactStore) Len() int {
	if s == nil {
		return 0
	}
	return s.length
}

func (s *symbolFactStore) Range(visit func(SymbolFact)) {
	if s == nil {
		return
	}
	for _, chunk := range s.chunks {
		for _, fact := range chunk {
			visit(fact)
		}
	}
}

func (s *symbolFactStore) Get(id SymbolID) (SymbolFact, bool) {
	if s == nil || len(s.chunks) == 0 {
		return SymbolFact{}, false
	}
	chunkIndex := s.chunkFor(id)
	chunk := s.chunks[chunkIndex]
	index := sort.Search(len(chunk), func(index int) bool { return chunk[index].ID >= id })
	if index == len(chunk) || chunk[index].ID != id {
		return SymbolFact{}, false
	}
	return chunk[index], true
}

func (s *symbolFactStore) chunkFor(id SymbolID) int {
	index := sort.Search(len(s.chunks), func(index int) bool {
		chunk := s.chunks[index]
		return chunk[len(chunk)-1].ID >= id
	})
	if index == len(s.chunks) {
		return len(s.chunks) - 1
	}
	return index
}

// Patch returns a new immutable generation. present is the complete current
// symbol universe; patches contains current rows for every added or changed
// symbol. The returned integer counts chunks shared with the preceding store.
func (s *symbolFactStore) Patch(
	present *symbolHandleSet,
	patches map[SymbolID]SymbolFact,
	removalCandidates map[SymbolID]struct{},
) (*symbolFactStore, int, []SymbolID, bool) {
	if s == nil || len(s.chunks) == 0 {
		facts := make([]SymbolFact, 0, len(patches))
		for _, fact := range patches {
			facts = append(facts, fact)
		}
		sort.Slice(facts, func(i, j int) bool { return facts[i].ID < facts[j].ID })
		store := newSymbolFactStore(facts)
		return store, 0, nil, store.Len() == present.len()
	}

	additions := make(map[int][]SymbolFact)
	affected := make(map[int]struct{})
	for id, fact := range patches {
		if _, exists := s.Get(id); exists {
			affected[s.chunkFor(id)] = struct{}{}
			continue
		}
		chunkIndex := s.chunkFor(id)
		additions[chunkIndex] = append(additions[chunkIndex], fact)
		affected[chunkIndex] = struct{}{}
	}
	var removed []SymbolID
	removedSet := make(map[SymbolID]struct{})
	for id := range removalCandidates {
		if present.containsID(id) {
			continue
		}
		if _, exists := s.Get(id); !exists {
			continue
		}
		removed = append(removed, id)
		removedSet[id] = struct{}{}
		affected[s.chunkFor(id)] = struct{}{}
	}

	next := &symbolFactStore{chunks: make([][]SymbolFact, 0, len(s.chunks)+len(additions))}
	shared := 0
	for chunkIndex, chunk := range s.chunks {
		if _, changed := affected[chunkIndex]; !changed {
			next.chunks = append(next.chunks, chunk)
			next.length += len(chunk)
			shared++
			continue
		}

		rows := make([]SymbolFact, 0, len(chunk)+len(additions[chunkIndex]))
		for _, fact := range chunk {
			if _, removed := removedSet[fact.ID]; removed {
				continue
			}
			if replacement, patched := patches[fact.ID]; patched {
				fact = replacement
			}
			rows = append(rows, fact)
		}
		rows = append(rows, additions[chunkIndex]...)
		sort.Slice(rows, func(i, j int) bool { return rows[i].ID < rows[j].ID })
		for start := 0; start < len(rows); start += symbolFactChunkSize {
			end := min(start+symbolFactChunkSize, len(rows))
			next.chunks = append(next.chunks, rows[start:end])
			next.length += end - start
		}
	}
	return next, shared, removed, next.Len() == present.len()
}

// PatchExisting replaces rows in a proven-stable symbol universe. It refuses
// additions so the caller's fixed-point proof cannot accidentally publish an
// incomplete store.
func (s *symbolFactStore) PatchExisting(patches map[SymbolID]SymbolFact) (*symbolFactStore, int, bool) {
	if s == nil {
		return nil, 0, false
	}
	affected := make(map[int]struct{}, len(patches))
	for id := range patches {
		if _, present := s.Get(id); !present {
			return nil, 0, false
		}
		affected[s.chunkFor(id)] = struct{}{}
	}
	next := &symbolFactStore{
		chunks: make([][]SymbolFact, len(s.chunks)),
		length: s.length,
	}
	shared := 0
	for chunkIndex, chunk := range s.chunks {
		if _, changed := affected[chunkIndex]; !changed {
			next.chunks[chunkIndex] = chunk
			shared++
			continue
		}
		rows := append([]SymbolFact(nil), chunk...)
		for index := range rows {
			if replacement, changed := patches[rows[index].ID]; changed {
				rows[index] = replacement
			}
		}
		next.chunks[chunkIndex] = rows
	}
	return next, shared, true
}

func (t FactTable) symbolFactsCount() int {
	if t.symbols != nil {
		return t.symbols.Len()
	}
	return len(t.Symbols)
}

func (t FactTable) rangeSymbolFacts(visit func(SymbolFact)) {
	if t.symbols != nil {
		t.symbols.Range(visit)
		return
	}
	for _, fact := range t.Symbols {
		visit(fact)
	}
}

func (t FactTable) canonicalSymbol(id SymbolID) (SymbolFact, bool) {
	if t.symbols != nil {
		return t.symbols.Get(id)
	}
	return canonicalSymbolFact(t.Symbols, id)
}
