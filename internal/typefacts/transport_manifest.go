package typefacts

import (
	"path/filepath"
	"sort"
)

const transportManifestPathLimit = 64

// transportManifest records the exact rows which may differ from the
// immediately preceding semantic-demand table. It is private implementation
// metadata: the frozen wire schema remains unchanged.
func transportManifest(previous, next *FactTable, builder *closureBuilder, changedPaths map[string]struct{}) *factTableTransportChanges {
	if previous == nil || builder == nil || !builder.referenceChangesExact || len(changedPaths) > transportManifestPathLimit {
		return nil
	}
	manifest := &factTableTransportChanges{
		baseGeneration: previous.Generation,
		sourcePaths:    make(map[string]struct{}, len(changedPaths)),
		entityPaths:    make(map[string]struct{}, len(changedPaths)),
		filePaths:      make(map[string]struct{}, len(changedPaths)),
		symbolIDs:      make(map[SymbolID]struct{}, len(builder.changedSymbolIDs)),
		exact:          true,
	}
	for path := range changedPaths {
		path = filepath.Clean(path)
		manifest.sourcePaths[path] = struct{}{}
		manifest.entityPaths[path] = struct{}{}
		manifest.filePaths[path] = struct{}{}
		collectPathSymbols(previous, path, manifest.symbolIDs)
		collectPathSymbols(next, path, manifest.symbolIDs)
	}
	for id := range builder.changedSymbolIDs {
		manifest.symbolIDs[id] = struct{}{}
	}

	// Symbol closure follows only alias edges. Expanding old and new targets
	// from every changed seed therefore captures additions, removals, and
	// full-tier reference changes without walking unrelated symbol rows.
	queue := make([]SymbolID, 0, len(manifest.symbolIDs))
	for id := range manifest.symbolIDs {
		queue = append(queue, id)
	}
	for index := 0; index < len(queue); index++ {
		id := queue[index]
		for _, table := range []*FactTable{previous, next} {
			if fact, ok := canonicalSymbolFact(table.Symbols, id); ok && fact.AliasTarget != "" {
				if _, seen := manifest.symbolIDs[fact.AliasTarget]; !seen {
					manifest.symbolIDs[fact.AliasTarget] = struct{}{}
					queue = append(queue, fact.AliasTarget)
				}
			}
		}
	}
	return manifest
}

func collectPathSymbols(table *FactTable, path string, symbols map[SymbolID]struct{}) {
	start := sort.Search(len(table.Entities), func(index int) bool {
		return table.Entities[index].Location.Path >= path
	})
	for index := start; index < len(table.Entities) && table.Entities[index].Location.Path == path; index++ {
		entity := table.Entities[index]
		if entity.Symbol != "" {
			symbols[entity.Symbol] = struct{}{}
		}
		if entity.ResolvedCall != nil && entity.ResolvedCall.Target != "" {
			symbols[entity.ResolvedCall.Target] = struct{}{}
		}
	}
	if file, ok := canonicalFileFact(table.Files, path); ok {
		for _, function := range file.AsyncFunctions {
			if function.Symbol != "" {
				symbols[function.Symbol] = struct{}{}
			}
			if function.Target != "" {
				symbols[function.Target] = struct{}{}
			}
		}
	}
}

func canonicalSymbolFact(facts []SymbolFact, id SymbolID) (SymbolFact, bool) {
	index := sort.Search(len(facts), func(index int) bool { return facts[index].ID >= id })
	if index == len(facts) || facts[index].ID != id {
		return SymbolFact{}, false
	}
	return facts[index], true
}

func canonicalFileFact(facts []FileFact, path string) (FileFact, bool) {
	index := sort.Search(len(facts), func(index int) bool { return facts[index].Path >= path })
	if index == len(facts) || facts[index].Path != path {
		return FileFact{}, false
	}
	return facts[index], true
}
