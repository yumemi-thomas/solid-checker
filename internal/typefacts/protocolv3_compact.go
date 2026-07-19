package typefacts

import "fmt"

// Compact v3 full-frame encoding.
//
// Cold analyze exchanges dominate boundary bytes because the plain wire
// shapes repeat CBOR field-name keys on every record and the absolute source
// path on every location. The compact forms carry one string dictionary per
// frame and encode rows as fixed-arity arrays; they decode into exactly the
// plain shapes, so everything past the transport seam is unchanged. Both
// executables ship in build-ID lockstep (the handshake rejects a mismatch),
// so the compact forms need no runtime negotiation.
//
// Dictionary index 0 is reserved for the empty string, which is also how
// optional string fields encode their absence. Optional nested rows encode
// as zero-or-one-element arrays and collections always encode as arrays —
// never null, which the deterministic CBOR contract forbids.

// CompactLocationV3 is [path-index, startByte, endByte].
type CompactLocationV3 struct {
	_         struct{} `cbor:",toarray"`
	Path      uint64
	StartByte uint64
	EndByte   uint64
}

// CompactDemandV3 is [flags, startByte, endByte, query-location-or-empty].
type CompactDemandV3 struct {
	_         struct{} `cbor:",toarray"`
	Flags     uint64
	StartByte uint64
	EndByte   uint64
	Query     []CompactLocationV3
}

// CompactDemandGroupV3 is [path-index, demand rows for that path].
type CompactDemandGroupV3 struct {
	_       struct{} `cbor:",toarray"`
	Path    uint64
	Demands []CompactDemandV3
}

// CompactDemandsV3 is the compact form of a full demand snapshot.
type CompactDemandsV3 struct {
	Groups  []CompactDemandGroupV3 `cbor:"groups" json:"groups"`
	Strings []string               `cbor:"strings" json:"strings"`
}

// Demand flag bits shared with the Rust encoder.
const (
	demandFlagSymbol             = 1 << 0
	demandFlagReferences         = 1 << 1
	demandFlagTypeDescriptor     = 1 << 2
	demandFlagResolvedCall       = 1 << 3
	demandFlagAsync              = 1 << 4
	demandFlagStructuralAccessor = 1 << 5
	demandFlagType               = 1 << 6
	demandFlagResolveAlias       = 1 << 7
	demandFlagDeclarations       = 1 << 8
)

// CompactDeclarationV3 is [name-index, kind-index, location].
type CompactDeclarationV3 struct {
	_        struct{} `cbor:",toarray"`
	Name     uint64
	Kind     uint64
	Location CompactLocationV3
}

// CompactTypeDescriptorV3 is [text-index, originModule-index, aliasDeclarations].
type CompactTypeDescriptorV3 struct {
	_                 struct{} `cbor:",toarray"`
	Text              uint64
	OriginModule      uint64
	AliasDeclarations []CompactDeclarationV3
}

// CompactCallV3 is [target-index, returnTypeText-index].
type CompactCallV3 struct {
	_              struct{} `cbor:",toarray"`
	Target         uint64
	ReturnTypeText uint64
}

// CompactEntityFactV3 is [startByte, endByte, symbol-index,
// descriptor-or-empty, resolvedCall-or-empty]; the path lives on the
// enclosing entity file.
type CompactEntityFactV3 struct {
	_              struct{} `cbor:",toarray"`
	StartByte      uint64
	EndByte        uint64
	Symbol         uint64
	TypeDescriptor []CompactTypeDescriptorV3
	ResolvedCall   []CompactCallV3
}

// CompactEntityFileV3 is [path-index, entity rows for that path].
type CompactEntityFileV3 struct {
	_        struct{} `cbor:",toarray"`
	Path     uint64
	Entities []CompactEntityFactV3
}

// CompactSymbolFactV3 is [id-index, aliasTarget-index, declarations, references].
type CompactSymbolFactV3 struct {
	_            struct{} `cbor:",toarray"`
	ID           uint64
	AliasTarget  uint64
	Declarations []CompactDeclarationV3
	References   []CompactLocationV3
}

// CompactSourceCallV3 is [location, callee, arguments, target-index].
type CompactSourceCallV3 struct {
	_         struct{} `cbor:",toarray"`
	Location  CompactLocationV3
	Callee    CompactLocationV3
	Arguments []CompactLocationV3
	Target    uint64
}

// CompactSourceBindingV3 is [flags, names, initializer]; flag bit 0 is array.
type CompactSourceBindingV3 struct {
	_           struct{} `cbor:",toarray"`
	Flags       uint64
	Names       []CompactLocationV3
	Initializer CompactSourceCallV3
}

const bindingFlagArray = 1 << 0

// CompactSourceFunctionV3 is [name, body, parameters, flags]; flag bits are
// exported, async, arrow.
type CompactSourceFunctionV3 struct {
	_          struct{} `cbor:",toarray"`
	Name       CompactLocationV3
	Body       CompactLocationV3
	Parameters []CompactLocationV3
	Flags      uint64
}

const (
	functionFlagExported = 1 << 0
	functionFlagAsync    = 1 << 1
	functionFlagArrow    = 1 << 2
)

// CompactAsyncFunctionV3 is [expression, symbol-index, target-index, flags,
// callsAfterAwait]; flag bit 0 is canReturnAsync.
type CompactAsyncFunctionV3 struct {
	_               struct{} `cbor:",toarray"`
	Expression      CompactLocationV3
	Symbol          uint64
	Target          uint64
	Flags           uint64
	CallsAfterAwait []CompactLocationV3
}

const asyncFunctionFlagCanReturnAsync = 1 << 0

// CompactFileFactV3 is [path-index, calls, bindings, functions, asyncFunctions].
type CompactFileFactV3 struct {
	_              struct{} `cbor:",toarray"`
	Path           uint64
	Calls          []CompactSourceCallV3
	Bindings       []CompactSourceBindingV3
	Functions      []CompactSourceFunctionV3
	AsyncFunctions []CompactAsyncFunctionV3
}

// CompactSourceDigestV3 is [path-index, sha256].
type CompactSourceDigestV3 struct {
	_      struct{} `cbor:",toarray"`
	Path   uint64
	SHA256 string
}

// CompactFactTableV3 is the compact form of a full v2-shaped fact table.
type CompactFactTableV3 struct {
	Schema      uint64                  `cbor:"schema" json:"schema"`
	Generation  uint64                  `cbor:"generation" json:"generation"`
	ProjectID   string                  `cbor:"projectId" json:"projectId"`
	Strings     []string                `cbor:"strings" json:"strings"`
	Sources     []CompactSourceDigestV3 `cbor:"sources" json:"sources"`
	EntityFiles []CompactEntityFileV3   `cbor:"entityFiles" json:"entityFiles"`
	Symbols     []CompactSymbolFactV3   `cbor:"symbols" json:"symbols"`
	Files       []CompactFileFactV3     `cbor:"files" json:"files"`
}

// stringTableV3 interns strings in first-occurrence order; index 0 is "".
type stringTableV3 struct {
	indexes map[string]uint64
	values  []string
}

func newStringTableV3() *stringTableV3 {
	return &stringTableV3{indexes: map[string]uint64{"": 0}, values: []string{""}}
}

func (t *stringTableV3) intern(value string) uint64 {
	if index, ok := t.indexes[value]; ok {
		return index
	}
	index := uint64(len(t.values))
	t.indexes[value] = index
	t.values = append(t.values, value)
	return index
}

type stringUntableV3 []string

func (t stringUntableV3) lookup(index uint64) (string, error) {
	if index >= uint64(len(t)) {
		return "", fmt.Errorf("compact string index %d out of range (%d strings)", index, len(t))
	}
	return t[index], nil
}

// CompactFactTableV3From converts a plain full table into its compact form.
func CompactFactTableV3From(table FactTableV2) CompactFactTableV3 {
	strings := newStringTableV3()
	location := func(l LocationV2) CompactLocationV3 {
		return CompactLocationV3{Path: strings.intern(l.Path), StartByte: l.StartByte, EndByte: l.EndByte}
	}
	locations := func(list []LocationV2) []CompactLocationV3 {
		out := make([]CompactLocationV3, 0, len(list))
		for _, l := range list {
			out = append(out, location(l))
		}
		return out
	}
	declarations := func(list []DeclarationV2) []CompactDeclarationV3 {
		out := make([]CompactDeclarationV3, 0, len(list))
		for _, d := range list {
			out = append(out, CompactDeclarationV3{
				Name:     strings.intern(d.Name),
				Kind:     strings.intern(d.Kind),
				Location: location(d.Location),
			})
		}
		return out
	}
	sourceCall := func(c SourceCallV2) CompactSourceCallV3 {
		return CompactSourceCallV3{
			Location:  location(c.Location),
			Callee:    location(c.Callee),
			Arguments: locations(c.Arguments),
			Target:    strings.intern(c.Target),
		}
	}

	compact := CompactFactTableV3{
		Schema:      table.Schema,
		Generation:  table.Generation,
		ProjectID:   table.ProjectID,
		Sources:     make([]CompactSourceDigestV3, 0, len(table.Sources)),
		EntityFiles: make([]CompactEntityFileV3, 0, 16),
		Symbols:     make([]CompactSymbolFactV3, 0, len(table.Symbols)),
		Files:       make([]CompactFileFactV3, 0, len(table.Files)),
	}
	for _, source := range table.Sources {
		compact.Sources = append(compact.Sources, CompactSourceDigestV3{
			Path:   strings.intern(source.Path),
			SHA256: source.SHA256,
		})
	}
	for _, entity := range table.Entities {
		path := strings.intern(entity.Location.Path)
		if len(compact.EntityFiles) == 0 || compact.EntityFiles[len(compact.EntityFiles)-1].Path != path {
			compact.EntityFiles = append(compact.EntityFiles, CompactEntityFileV3{
				Path:     path,
				Entities: make([]CompactEntityFactV3, 0, 8),
			})
		}
		row := CompactEntityFactV3{
			StartByte:      entity.Location.StartByte,
			EndByte:        entity.Location.EndByte,
			Symbol:         strings.intern(entity.Symbol),
			TypeDescriptor: []CompactTypeDescriptorV3{},
			ResolvedCall:   []CompactCallV3{},
		}
		if entity.TypeDescriptor != nil {
			row.TypeDescriptor = append(row.TypeDescriptor, CompactTypeDescriptorV3{
				Text:              strings.intern(entity.TypeDescriptor.Text),
				OriginModule:      strings.intern(entity.TypeDescriptor.OriginModule),
				AliasDeclarations: declarations(entity.TypeDescriptor.AliasDeclarations),
			})
		}
		if entity.ResolvedCall != nil {
			row.ResolvedCall = append(row.ResolvedCall, CompactCallV3{
				Target:         strings.intern(entity.ResolvedCall.Target),
				ReturnTypeText: strings.intern(entity.ResolvedCall.ReturnTypeText),
			})
		}
		group := &compact.EntityFiles[len(compact.EntityFiles)-1]
		group.Entities = append(group.Entities, row)
	}
	for _, symbol := range table.Symbols {
		compact.Symbols = append(compact.Symbols, CompactSymbolFactV3{
			ID:           strings.intern(symbol.ID),
			AliasTarget:  strings.intern(symbol.AliasTarget),
			Declarations: declarations(symbol.Declarations),
			References:   locations(symbol.References),
		})
	}
	for _, file := range table.Files {
		calls := make([]CompactSourceCallV3, 0, len(file.Calls))
		for _, call := range file.Calls {
			calls = append(calls, sourceCall(call))
		}
		bindings := make([]CompactSourceBindingV3, 0, len(file.Bindings))
		for _, binding := range file.Bindings {
			flags := uint64(0)
			if binding.Array {
				flags |= bindingFlagArray
			}
			bindings = append(bindings, CompactSourceBindingV3{
				Flags:       flags,
				Names:       locations(binding.Names),
				Initializer: sourceCall(binding.Initializer),
			})
		}
		functions := make([]CompactSourceFunctionV3, 0, len(file.Functions))
		for _, function := range file.Functions {
			flags := uint64(0)
			if function.Exported {
				flags |= functionFlagExported
			}
			if function.Async {
				flags |= functionFlagAsync
			}
			if function.Arrow {
				flags |= functionFlagArrow
			}
			functions = append(functions, CompactSourceFunctionV3{
				Name:       location(function.Name),
				Body:       location(function.Body),
				Parameters: locations(function.Parameters),
				Flags:      flags,
			})
		}
		asyncFunctions := make([]CompactAsyncFunctionV3, 0, len(file.AsyncFunctions))
		for _, function := range file.AsyncFunctions {
			flags := uint64(0)
			if function.CanReturnAsync {
				flags |= asyncFunctionFlagCanReturnAsync
			}
			asyncFunctions = append(asyncFunctions, CompactAsyncFunctionV3{
				Expression:      location(function.Expression),
				Symbol:          strings.intern(function.Symbol),
				Target:          strings.intern(function.Target),
				Flags:           flags,
				CallsAfterAwait: locations(function.CallsAfterAwait),
			})
		}
		compact.Files = append(compact.Files, CompactFileFactV3{
			Path:           strings.intern(file.Path),
			Calls:          calls,
			Bindings:       bindings,
			Functions:      functions,
			AsyncFunctions: asyncFunctions,
		})
	}
	compact.Strings = strings.values
	return compact
}

// Expand converts the compact table back into the plain full table. Every
// dictionary reference is bounds-checked; a gap fails the frame closed.
func (compact CompactFactTableV3) Expand() (FactTableV2, error) {
	strings := stringUntableV3(compact.Strings)
	location := func(l CompactLocationV3) (LocationV2, error) {
		path, err := strings.lookup(l.Path)
		if err != nil {
			return LocationV2{}, err
		}
		return LocationV2{Path: path, StartByte: l.StartByte, EndByte: l.EndByte}, nil
	}
	locations := func(list []CompactLocationV3) ([]LocationV2, error) {
		if len(list) == 0 {
			return nil, nil
		}
		out := make([]LocationV2, 0, len(list))
		for _, l := range list {
			expanded, err := location(l)
			if err != nil {
				return nil, err
			}
			out = append(out, expanded)
		}
		return out, nil
	}
	declarations := func(list []CompactDeclarationV3) ([]DeclarationV2, error) {
		if len(list) == 0 {
			return nil, nil
		}
		out := make([]DeclarationV2, 0, len(list))
		for _, d := range list {
			name, err := strings.lookup(d.Name)
			if err != nil {
				return nil, err
			}
			kind, err := strings.lookup(d.Kind)
			if err != nil {
				return nil, err
			}
			expanded, err := location(d.Location)
			if err != nil {
				return nil, err
			}
			out = append(out, DeclarationV2{Name: name, Kind: kind, Location: expanded})
		}
		return out, nil
	}
	sourceCall := func(c CompactSourceCallV3) (SourceCallV2, error) {
		callLocation, err := location(c.Location)
		if err != nil {
			return SourceCallV2{}, err
		}
		callee, err := location(c.Callee)
		if err != nil {
			return SourceCallV2{}, err
		}
		arguments, err := locations(c.Arguments)
		if err != nil {
			return SourceCallV2{}, err
		}
		target, err := strings.lookup(c.Target)
		if err != nil {
			return SourceCallV2{}, err
		}
		return SourceCallV2{Location: callLocation, Callee: callee, Arguments: arguments, Target: target}, nil
	}

	table := FactTableV2{
		Schema:     compact.Schema,
		Generation: compact.Generation,
		ProjectID:  compact.ProjectID,
		Sources:    make([]SourceDigestV2, 0, len(compact.Sources)),
		Entities:   make([]EntityFactV2, 0, 16),
		Symbols:    make([]SymbolFactV2, 0, len(compact.Symbols)),
		Files:      make([]FileFactV2, 0, len(compact.Files)),
	}
	for _, source := range compact.Sources {
		path, err := strings.lookup(source.Path)
		if err != nil {
			return FactTableV2{}, err
		}
		table.Sources = append(table.Sources, SourceDigestV2{Path: path, SHA256: source.SHA256})
	}
	for _, group := range compact.EntityFiles {
		path, err := strings.lookup(group.Path)
		if err != nil {
			return FactTableV2{}, err
		}
		for _, row := range group.Entities {
			symbol, err := strings.lookup(row.Symbol)
			if err != nil {
				return FactTableV2{}, err
			}
			if len(row.TypeDescriptor) > 1 || len(row.ResolvedCall) > 1 {
				return FactTableV2{}, fmt.Errorf("compact optional entity row carries more than one element")
			}
			entity := EntityFactV2{
				Location: LocationV2{Path: path, StartByte: row.StartByte, EndByte: row.EndByte},
				Symbol:   symbol,
			}
			if len(row.TypeDescriptor) == 1 {
				descriptor := row.TypeDescriptor[0]
				text, err := strings.lookup(descriptor.Text)
				if err != nil {
					return FactTableV2{}, err
				}
				origin, err := strings.lookup(descriptor.OriginModule)
				if err != nil {
					return FactTableV2{}, err
				}
				alias, err := declarations(descriptor.AliasDeclarations)
				if err != nil {
					return FactTableV2{}, err
				}
				entity.TypeDescriptor = &TypeDescriptorV2{Text: text, OriginModule: origin, AliasDeclarations: alias}
			}
			if len(row.ResolvedCall) == 1 {
				call := row.ResolvedCall[0]
				target, err := strings.lookup(call.Target)
				if err != nil {
					return FactTableV2{}, err
				}
				returnType, err := strings.lookup(call.ReturnTypeText)
				if err != nil {
					return FactTableV2{}, err
				}
				entity.ResolvedCall = &CallV2{Target: target, ReturnTypeText: returnType}
			}
			table.Entities = append(table.Entities, entity)
		}
	}
	for _, symbol := range compact.Symbols {
		id, err := strings.lookup(symbol.ID)
		if err != nil {
			return FactTableV2{}, err
		}
		aliasTarget, err := strings.lookup(symbol.AliasTarget)
		if err != nil {
			return FactTableV2{}, err
		}
		symbolDeclarations, err := declarations(symbol.Declarations)
		if err != nil {
			return FactTableV2{}, err
		}
		references, err := locations(symbol.References)
		if err != nil {
			return FactTableV2{}, err
		}
		table.Symbols = append(table.Symbols, SymbolFactV2{
			ID:           id,
			AliasTarget:  aliasTarget,
			Declarations: symbolDeclarations,
			References:   references,
		})
	}
	for _, file := range compact.Files {
		path, err := strings.lookup(file.Path)
		if err != nil {
			return FactTableV2{}, err
		}
		expanded := FileFactV2{Path: path}
		if len(file.Calls) != 0 {
			expanded.Calls = make([]SourceCallV2, 0, len(file.Calls))
			for _, call := range file.Calls {
				plain, err := sourceCall(call)
				if err != nil {
					return FactTableV2{}, err
				}
				expanded.Calls = append(expanded.Calls, plain)
			}
		}
		if len(file.Bindings) != 0 {
			expanded.Bindings = make([]SourceBindingV2, 0, len(file.Bindings))
			for _, binding := range file.Bindings {
				names, err := locations(binding.Names)
				if err != nil {
					return FactTableV2{}, err
				}
				initializer, err := sourceCall(binding.Initializer)
				if err != nil {
					return FactTableV2{}, err
				}
				expanded.Bindings = append(expanded.Bindings, SourceBindingV2{
					Array:       binding.Flags&bindingFlagArray != 0,
					Names:       names,
					Initializer: initializer,
				})
			}
		}
		if len(file.Functions) != 0 {
			expanded.Functions = make([]SourceFunctionV2, 0, len(file.Functions))
			for _, function := range file.Functions {
				name, err := location(function.Name)
				if err != nil {
					return FactTableV2{}, err
				}
				body, err := location(function.Body)
				if err != nil {
					return FactTableV2{}, err
				}
				parameters, err := locations(function.Parameters)
				if err != nil {
					return FactTableV2{}, err
				}
				expanded.Functions = append(expanded.Functions, SourceFunctionV2{
					Name:       name,
					Body:       body,
					Parameters: parameters,
					Exported:   function.Flags&functionFlagExported != 0,
					Async:      function.Flags&functionFlagAsync != 0,
					Arrow:      function.Flags&functionFlagArrow != 0,
				})
			}
		}
		if len(file.AsyncFunctions) != 0 {
			expanded.AsyncFunctions = make([]AsyncFunctionFactV2, 0, len(file.AsyncFunctions))
			for _, function := range file.AsyncFunctions {
				expression, err := location(function.Expression)
				if err != nil {
					return FactTableV2{}, err
				}
				symbol, err := strings.lookup(function.Symbol)
				if err != nil {
					return FactTableV2{}, err
				}
				target, err := strings.lookup(function.Target)
				if err != nil {
					return FactTableV2{}, err
				}
				callsAfterAwait, err := locations(function.CallsAfterAwait)
				if err != nil {
					return FactTableV2{}, err
				}
				expanded.AsyncFunctions = append(expanded.AsyncFunctions, AsyncFunctionFactV2{
					Expression:      expression,
					Symbol:          symbol,
					Target:          target,
					CanReturnAsync:  function.Flags&asyncFunctionFlagCanReturnAsync != 0,
					CallsAfterAwait: callsAfterAwait,
				})
			}
		}
		table.Files = append(table.Files, expanded)
	}
	return table, nil
}

// CompactDemandsV3From converts a full demand snapshot into its compact
// form. Demands are grouped by location path in input order.
func CompactDemandsV3From(demands []EntityDemand) CompactDemandsV3 {
	strings := newStringTableV3()
	compact := CompactDemandsV3{Groups: make([]CompactDemandGroupV3, 0, 16)}
	for _, demand := range demands {
		path := strings.intern(demand.Location.Path)
		if len(compact.Groups) == 0 || compact.Groups[len(compact.Groups)-1].Path != path {
			compact.Groups = append(compact.Groups, CompactDemandGroupV3{
				Path:    path,
				Demands: make([]CompactDemandV3, 0, 8),
			})
		}
		flags := uint64(0)
		if demand.Symbol {
			flags |= demandFlagSymbol
		}
		if demand.References {
			flags |= demandFlagReferences
		}
		if demand.TypeDescriptor {
			flags |= demandFlagTypeDescriptor
		}
		if demand.ResolvedCall {
			flags |= demandFlagResolvedCall
		}
		if demand.Async {
			flags |= demandFlagAsync
		}
		if demand.StructuralAccessor {
			flags |= demandFlagStructuralAccessor
		}
		if demand.Type {
			flags |= demandFlagType
		}
		if demand.ResolveAlias {
			flags |= demandFlagResolveAlias
		}
		if demand.Declarations {
			flags |= demandFlagDeclarations
		}
		row := CompactDemandV3{
			Flags:     flags,
			StartByte: uint64(demand.Location.StartByte),
			EndByte:   uint64(demand.Location.EndByte),
			Query:     []CompactLocationV3{},
		}
		if demand.QueryLocation != nil {
			row.Query = append(row.Query, CompactLocationV3{
				Path:      strings.intern(demand.QueryLocation.Path),
				StartByte: uint64(demand.QueryLocation.StartByte),
				EndByte:   uint64(demand.QueryLocation.EndByte),
			})
		}
		group := &compact.Groups[len(compact.Groups)-1]
		group.Demands = append(group.Demands, row)
	}
	compact.Strings = strings.values
	return compact
}

// Expand converts the compact demand snapshot back into plain demands.
func (compact CompactDemandsV3) Expand() ([]EntityDemand, error) {
	strings := stringUntableV3(compact.Strings)
	demands := make([]EntityDemand, 0, 64)
	for _, group := range compact.Groups {
		path, err := strings.lookup(group.Path)
		if err != nil {
			return nil, err
		}
		for _, row := range group.Demands {
			demand := EntityDemand{
				Location:           Location{Path: path, StartByte: int(row.StartByte), EndByte: int(row.EndByte)},
				Symbol:             row.Flags&demandFlagSymbol != 0,
				References:         row.Flags&demandFlagReferences != 0,
				TypeDescriptor:     row.Flags&demandFlagTypeDescriptor != 0,
				ResolvedCall:       row.Flags&demandFlagResolvedCall != 0,
				Async:              row.Flags&demandFlagAsync != 0,
				StructuralAccessor: row.Flags&demandFlagStructuralAccessor != 0,
				Type:               row.Flags&demandFlagType != 0,
				ResolveAlias:       row.Flags&demandFlagResolveAlias != 0,
				Declarations:       row.Flags&demandFlagDeclarations != 0,
			}
			if len(row.Query) > 1 {
				return nil, fmt.Errorf("compact demand query carries more than one element")
			}
			if len(row.Query) == 1 {
				queryPath, err := strings.lookup(row.Query[0].Path)
				if err != nil {
					return nil, err
				}
				demand.QueryLocation = &Location{
					Path:      queryPath,
					StartByte: int(row.Query[0].StartByte),
					EndByte:   int(row.Query[0].EndByte),
				}
			}
			demands = append(demands, demand)
		}
	}
	return demands, nil
}
