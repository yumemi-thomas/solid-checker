package typefacts

// Symbol interning. A SymbolID is a ~33-byte string, and the closure's
// per-generation bookkeeping used to key half a dozen freshly allocated maps
// by it — so every generation, including the patched fast path, paid string
// hashing and map growth proportional to the whole symbol universe. The
// interner assigns each SymbolID a dense session-stable handle once; the
// per-generation sets become bitsets over handles, backed by scratch the
// closure reuses across generations.

type symbolInterner struct {
	handles map[SymbolID]int32
	ids     []SymbolID
}

func newSymbolInterner() *symbolInterner {
	return &symbolInterner{handles: make(map[SymbolID]int32)}
}

// handle interns id, assigning the next dense handle on first sight.
func (n *symbolInterner) handle(id SymbolID) int32 {
	if handle, ok := n.handles[id]; ok {
		return handle
	}
	handle := int32(len(n.ids))
	n.handles[id] = handle
	n.ids = append(n.ids, id)
	return handle
}

// lookup reports id's handle without interning it.
func (n *symbolInterner) lookup(id SymbolID) (int32, bool) {
	handle, ok := n.handles[id]
	return handle, ok
}

func (n *symbolInterner) size() int {
	return len(n.ids)
}

// symbolHandleSet is one generation's symbol membership as a bitset over
// interned handles. add grows the backing as the interner grows mid-build.
type symbolHandleSet struct {
	interner *symbolInterner
	members  []bool
	count    int
}

// newSymbolHandleSet builds an empty set on top of scratch, growing it to the
// interner's current size. The caller reclaims the backing via members after
// the generation.
func newSymbolHandleSet(interner *symbolInterner, scratch []bool) *symbolHandleSet {
	if cap(scratch) < interner.size() {
		scratch = make([]bool, interner.size())
	} else {
		scratch = scratch[:cap(scratch)]
		clear(scratch)
	}
	return &symbolHandleSet{interner: interner, members: scratch}
}

// add reports whether handle was newly inserted.
func (s *symbolHandleSet) add(handle int32) bool {
	if int(handle) >= len(s.members) {
		grown := make([]bool, max(s.interner.size(), int(handle)+1))
		copy(grown, s.members)
		s.members = grown
	}
	if s.members[handle] {
		return false
	}
	s.members[handle] = true
	s.count++
	return true
}

func (s *symbolHandleSet) addID(id SymbolID) bool {
	return s.add(s.interner.handle(id))
}

func (s *symbolHandleSet) contains(handle int32) bool {
	return int(handle) < len(s.members) && s.members[handle]
}

// containsID reports membership without interning unknown IDs.
func (s *symbolHandleSet) containsID(id SymbolID) bool {
	handle, ok := s.interner.lookup(id)
	return ok && s.contains(handle)
}

func (s *symbolHandleSet) len() int {
	return s.count
}

// changedSymbolSet is a symbolHandleSet that also keeps the inserted IDs as a
// list, for the consumers that iterate the changed set (the transport
// manifest) or size by it.
type changedSymbolSet struct {
	set symbolHandleSet
	ids []SymbolID
}

func newChangedSymbolSet(interner *symbolInterner, scratch []bool, idScratch []SymbolID) *changedSymbolSet {
	return &changedSymbolSet{
		set: *newSymbolHandleSet(interner, scratch),
		ids: idScratch[:0],
	}
}

func (c *changedSymbolSet) add(id SymbolID) {
	if c.set.addID(id) {
		c.ids = append(c.ids, id)
	}
}

func (c *changedSymbolSet) containsID(id SymbolID) bool {
	return c.set.containsID(id)
}

func (c *changedSymbolSet) len() int {
	return len(c.ids)
}
