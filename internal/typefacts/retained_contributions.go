package typefacts

import (
	"fmt"
	"path/filepath"
	"sort"
)

// retainedContribution is the immutable canonical form of one file's share of
// a Demand closure. It owns every slice reachable from the retained store.
type retainedContribution struct {
	demandHash        uint64
	entities          []EntityFact
	fullTier          []uint32
	roots             []SymbolID
	fullTierSymbols   []SymbolID
	descriptors       []retainedDescriptor
	structural        []SymbolID
	dependencies      []string
	descriptorSymbols []SymbolID
	durable           bool
	seedsPrepared     bool
}

type retainedDescriptor struct {
	symbol     SymbolID
	descriptor *TypeDescriptor
}

// releaseTransportRows transfers expanded entity ownership to the v6 client.
// The compact semantic seeds are sufficient to prove and patch later closure
// generations; source locations and resolved-call bodies remain solely in
// Rust's retained table.
func (c *retainedContribution) prepareTransportSeeds() {
	if c == nil || c.entities == nil || c.seedsPrepared {
		return
	}
	for index := range c.entities {
		entity := &c.entities[index]
		if entity.Symbol != "" {
			c.roots = append(c.roots, entity.Symbol)
		}
		if entity.ResolvedCall != nil && entity.ResolvedCall.Target != "" {
			c.roots = append(c.roots, entity.ResolvedCall.Target)
		}
		if entity.ResolvedCall != nil && entity.ResolvedCall.Targets != nil {
			for candidateIndex := range entity.ResolvedCall.Targets.Candidates {
				if symbol := entity.ResolvedCall.Targets.Candidates[candidateIndex].Symbol; symbol != "" {
					c.roots = append(c.roots, symbol)
				}
			}
		}
		if entity.Symbol != "" && entity.TypeDescriptor != nil {
			c.descriptors = append(c.descriptors, retainedDescriptor{
				symbol: entity.Symbol, descriptor: entity.TypeDescriptor,
			})
		}
	}
	for _, entityIndex := range c.fullTier {
		if symbol := c.entities[entityIndex].Symbol; symbol != "" {
			c.fullTierSymbols = append(c.fullTierSymbols, symbol)
		}
	}
	c.seedsPrepared = true
}

func (c *retainedContribution) releaseTransportRows() {
	if c == nil || c.entities == nil {
		return
	}
	c.prepareTransportSeeds()
	c.entities = nil
	c.fullTier = nil
}

// prepareRetainedContribution merges an aligned Semantic demand-run result
// exactly once. Same-location demands are adjacent in canonical order, so no
// location map or post-build normalization is needed.
func prepareRetainedContribution(
	path string,
	hash uint64,
	demands []EntityDemand,
	result SemanticDemandRunResult,
) (*retainedContribution, error) {
	if len(result.Entities) != len(demands) || len(result.Structural) != len(demands) {
		return nil, fmt.Errorf(
			"semantic demand run %q returned %d entities and %d structural symbols for %d demands",
			path,
			len(result.Entities),
			len(result.Structural),
			len(demands),
		)
	}

	fullTierCount := 0
	for index := range demands {
		if result.Entities[index].Symbol != "" {
			if demands[index].References {
				fullTierCount++
			}
		}
	}

	contribution := &retainedContribution{
		demandHash: hash,
		// SemanticDemandRuns transfers this result exactly once. Compact
		// adjacent same-location demand rows in place so the contribution
		// takes ownership of TS-Go's entity arena instead of cloning it.
		entities: result.Entities[:0],
		fullTier: make([]uint32, 0, fullTierCount),
		// Structural evidence is consumed by this same transaction, so
		// compact its non-empty rows into the transferred TS-Go arena too.
		structural:   result.Structural[:0],
		dependencies: result.Dependencies,
		durable:      result.Durable,
	}
	if err := validateCanonicalDependencyPaths(path, contribution.dependencies); err != nil {
		return nil, err
	}
	for index := range demands {
		expected := demands[index].Location
		if result.Entities[index].Location != expected {
			return nil, fmt.Errorf(
				"semantic demand run %q entity %d location = %+v, want %+v",
				path,
				index,
				result.Entities[index].Location,
				expected,
			)
		}
		demand := &demands[index]
		// Copy before append: contribution.entities and result.Entities share
		// backing, and the write cursor may overwrite this source slot.
		entity := result.Entities[index]
		var target *EntityFact
		if last := len(contribution.entities) - 1; last >= 0 &&
			contribution.entities[last].Location == entity.Location {
			target = &contribution.entities[last]
		} else {
			contribution.entities = append(contribution.entities, EntityFact{Location: entity.Location})
			target = &contribution.entities[len(contribution.entities)-1]
		}
		if entity.Symbol != "" {
			target.Symbol = entity.Symbol
			target.SymbolUnresolved = false
			if demand.References {
				contribution.fullTier = append(contribution.fullTier, uint32(len(contribution.entities)-1))
			}
		} else if entity.SymbolUnresolved && target.Symbol == "" {
			target.SymbolUnresolved = true
		}
		if entity.TypeDescriptor != nil {
			target.TypeDescriptor = entity.TypeDescriptor
		}
		if entity.ResolvedCall != nil {
			target.ResolvedCall = entity.ResolvedCall
		}
		if entity.Callability != "" {
			target.Callability = entity.Callability
		}
		if entity.RuntimeValueDomain != nil {
			target.RuntimeValueDomain = entity.RuntimeValueDomain
		}
		if entity.ReferenceSpace != "" {
			target.ReferenceSpace = entity.ReferenceSpace
		}
		if entity.RuntimeIdentity != "" {
			target.RuntimeIdentity = entity.RuntimeIdentity
		}
		if result.Structural[index] != "" {
			contribution.structural = append(contribution.structural, result.Structural[index])
		}
	}

	descriptorSymbols := make([]SymbolID, 0)
	entityIndex := -1
	for index := range demands {
		// result.Entities has been compacted in place, so the immutable demand
		// run is the canonical source for original row boundaries here.
		if index == 0 || demands[index].Location != demands[index-1].Location {
			entityIndex++
		}
		if demands[index].TypeDescriptor {
			if symbol := contribution.entities[entityIndex].Symbol; symbol != "" {
				descriptorSymbols = append(descriptorSymbols, symbol)
			}
		}
	}
	sort.Slice(descriptorSymbols, func(i, j int) bool {
		return descriptorSymbols[i] < descriptorSymbols[j]
	})
	write := 0
	for _, symbol := range descriptorSymbols {
		if write != 0 && descriptorSymbols[write-1] == symbol {
			continue
		}
		descriptorSymbols[write] = symbol
		write++
	}
	contribution.descriptorSymbols = descriptorSymbols[:write:write]
	contribution.entities = contribution.entities[:len(contribution.entities):len(contribution.entities)]
	contribution.fullTier = contribution.fullTier[:len(contribution.fullTier):len(contribution.fullTier)]
	contribution.structural = contribution.structural[:len(contribution.structural):len(contribution.structural)]
	return contribution, nil
}

func validateCanonicalDependencyPaths(owner string, paths []string) error {
	if owner == "" || filepath.Clean(owner) != owner {
		return fmt.Errorf("semantic dependency owner %q is not clean and non-empty", owner)
	}
	for index, path := range paths {
		if path == "" || filepath.Clean(path) != path {
			return fmt.Errorf("semantic dependency path %q is not clean and non-empty", path)
		}
		if path == owner {
			return fmt.Errorf("semantic dependency paths include owner %q", owner)
		}
		if index != 0 && paths[index-1] >= path {
			return fmt.Errorf("semantic dependency paths are not strictly ordered at %q", path)
		}
	}
	return nil
}

// retainedContributionStore owns retained contributions and the two reverse
// indexes that make source invalidation and suppression refresh targeted.
type retainedContributionStore struct {
	byPath           map[string]*retainedContribution
	dependentsByPath map[string]pathMembership
	descriptorUsers  map[SymbolID]pathMembership
}

// pathMembership keeps the overwhelmingly common small reverse-index bucket
// compact, then promotes high fanout to constant-time unordered membership.
// Neither consumer observes bucket order; deterministic output is established
// only after membership has been translated to canonical group indices.
const pathMembershipMapThreshold = 16

type pathMembership struct {
	small []string
	large map[string]struct{}
}

func (m pathMembership) add(path string) pathMembership {
	if m.large != nil {
		m.large[path] = struct{}{}
		return m
	}
	for _, existing := range m.small {
		if existing == path {
			return m
		}
	}
	if len(m.small) < pathMembershipMapThreshold {
		m.small = append(m.small, path)
		return m
	}
	m.large = make(map[string]struct{}, len(m.small)+1)
	for _, existing := range m.small {
		m.large[existing] = struct{}{}
	}
	m.large[path] = struct{}{}
	m.small = nil
	return m
}

func (m pathMembership) remove(path string) pathMembership {
	if m.large != nil {
		delete(m.large, path)
		return m
	}
	for index, existing := range m.small {
		if existing != path {
			continue
		}
		last := len(m.small) - 1
		m.small[index] = m.small[last]
		m.small[last] = ""
		m.small = m.small[:last]
		break
	}
	return m
}

func (m pathMembership) len() int {
	if m.large != nil {
		return len(m.large)
	}
	return len(m.small)
}

func (m pathMembership) rangePaths(visit func(string)) {
	if m.large != nil {
		for path := range m.large {
			visit(path)
		}
		return
	}
	for _, path := range m.small {
		visit(path)
	}
}

func (s *retainedContributionStore) get(path string) *retainedContribution {
	return s.byPath[path]
}

func (s *retainedContributionStore) add(path string, contribution *retainedContribution) {
	if s.byPath == nil {
		s.byPath = make(map[string]*retainedContribution)
	}
	if previous := s.byPath[path]; previous != nil {
		s.remove(path)
	}
	s.byPath[path] = contribution
	for _, dependency := range contribution.dependencies {
		if s.dependentsByPath == nil {
			s.dependentsByPath = make(map[string]pathMembership)
		}
		s.dependentsByPath[dependency] = s.dependentsByPath[dependency].add(path)
	}
	for _, symbol := range contribution.descriptorSymbols {
		if s.descriptorUsers == nil {
			s.descriptorUsers = make(map[SymbolID]pathMembership)
		}
		s.descriptorUsers[symbol] = s.descriptorUsers[symbol].add(path)
	}
}

func (s *retainedContributionStore) remove(path string) {
	contribution := s.byPath[path]
	if contribution == nil {
		return
	}
	delete(s.byPath, path)
	for _, dependency := range contribution.dependencies {
		users := s.dependentsByPath[dependency].remove(path)
		if users.len() == 0 {
			delete(s.dependentsByPath, dependency)
		} else {
			s.dependentsByPath[dependency] = users
		}
	}
	for _, symbol := range contribution.descriptorSymbols {
		users := s.descriptorUsers[symbol].remove(path)
		if users.len() == 0 {
			delete(s.descriptorUsers, symbol)
		} else {
			s.descriptorUsers[symbol] = users
		}
	}
}

// invalidate removes the named paths and their direct dependents. It extends
// paths with those dependents so the Transport manifest names every recomputed
// contribution, but deliberately does not recurse beyond the source evidence
// the previous implementation used.
func (s *retainedContributionStore) invalidate(paths map[string]struct{}) {
	direct := make([]string, 0, len(paths))
	for path := range paths {
		direct = append(direct, path)
	}
	for _, path := range direct {
		s.dependentsByPath[path].rangePaths(func(dependent string) {
			paths[dependent] = struct{}{}
		})
	}
	for path := range paths {
		s.remove(path)
	}
}

// discard removes only the named Demand runs. Changing what one file asks for
// cannot change a different file's source-derived semantic facts.
func (s *retainedContributionStore) discard(paths map[string]struct{}) {
	for path := range paths {
		s.remove(path)
	}
}

func (s *retainedContributionStore) rangeDescriptorUsers(symbol SymbolID, visit func(string)) {
	s.descriptorUsers[symbol].rangePaths(visit)
}

// commit publishes a Retained contribution transaction. It is called
// only after semantic resolution, symbol closure, assembly and the Transport
// manifest have all succeeded.
func (s *retainedContributionStore) commit(
	groups []demandGroup,
	desired map[string]struct{},
) {
	clear(desired)
	for index := range groups {
		group := &groups[index]
		if group.contribution != nil && group.contribution.durable {
			desired[group.path] = struct{}{}
		}
	}
	for path := range s.byPath {
		if _, keep := desired[path]; !keep {
			s.remove(path)
		}
	}
	for index := range groups {
		group := &groups[index]
		if group.contribution == nil || !group.contribution.durable || s.byPath[group.path] == group.contribution {
			continue
		}
		s.add(group.path, group.contribution)
	}
	clear(desired)
}

func (s *retainedContributionStore) reset() {
	s.byPath = nil
	s.dependentsByPath = nil
	s.descriptorUsers = nil
}
