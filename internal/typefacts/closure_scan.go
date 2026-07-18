package typefacts

import "regexp"

// These conservative demand-closure seed scans predate authoritative Oxc
// locations. They remain as a compatibility fallback; the Rust checker sends
// Oxc-derived seeds in production.
var (
	closureExportConstPattern     = regexp.MustCompile(`(?m)export\s+const\s+`)
	closureExportClassPattern     = regexp.MustCompile(`(?m)export\s+class\s+([A-Za-z_$][A-Za-z0-9_$]*)`)
	closureExportListPattern      = regexp.MustCompile(`(?m)export\s*\{([^}]*)\}`)
	closureImportSpecifierPattern = regexp.MustCompile(`^\s*([A-Za-z_$][A-Za-z0-9_$]*)(?:\s+as\s+([A-Za-z_$][A-Za-z0-9_$]*))?\s*$`)
	closureAliasAssignmentPattern = regexp.MustCompile(`(?m)\bconst\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*([A-Za-z_$][A-Za-z0-9_$]*)\s*;`)
	// Const declaration names, including array-destructure slots. reactiveir's
	// binding chains (signal/store/setter/factory patterns) find these by
	// byte scan and query their symbols even when a type assertion around
	// the initializer hides the call from the SourceBindings table
	// (`const [s, set] = createSignal(...) as unknown as [...]`).
	closureConstBindingPattern    = regexp.MustCompile(`(?m)(?:export\s+)?\bconst\s*(\[[^\]\n]*\]|[A-Za-z_$][A-Za-z0-9_$]*)\s*=`)
	closureNamedImportPattern     = regexp.MustCompile(`(?m)import\s*\{([^}]*)\}\s*from\s*["']([^"']+)["']`)
	closureIdentifierPattern      = regexp.MustCompile(`^[A-Za-z_$][A-Za-z0-9_$]*$`)
	closureIdentifierTokenPattern = regexp.MustCompile(`[A-Za-z_$][A-Za-z0-9_$]*`)
	// Braced bare identifiers and element tags approximate the ExecutionMap
	// spans (callback roles, JSX operations, execution regions) reactiveir
	// derives identifier queries from. The closure over-approximates with a
	// byte scan until compiler-facts spans become a first-class seed input.
	closureBracedIdentifierPattern = regexp.MustCompile(`\{\s*([A-Za-z_$][A-Za-z0-9_$]*)\s*\}`)
	closureJSXTagPattern           = regexp.MustCompile(`<([A-Za-z][A-Za-z0-9_$]*)`)
)

type closureByteSpan struct {
	start, end int
}

func compilerSpanIdentifiers(path string, source []byte, span Location) []Location {
	start, end := span.StartByte, span.EndByte
	if start < 0 || end > len(source) || start >= end {
		return nil
	}
	var result []Location
	for index := start; index < end; {
		if !identifierStart(source[index]) {
			index++
			continue
		}
		finish := index + 1
		for finish < end && identifierContinue(source[finish]) {
			finish++
		}
		result = append(result, Location{Path: path, StartByte: index, EndByte: finish})
		index = finish
	}
	return result
}

func identifierStart(value byte) bool {
	return value == '_' || value == '$' || value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func identifierContinue(value byte) bool {
	return identifierStart(value) || value >= '0' && value <= '9'
}

func closureTrimByteSpan(source []byte, start, end int) (int, int) {
	for start < end && (source[start] == ' ' || source[start] == '\t' || source[start] == '\r' || source[start] == '\n') {
		start++
	}
	for end > start && (source[end-1] == ' ' || source[end-1] == '\t' || source[end-1] == '\r' || source[end-1] == '\n') {
		end--
	}
	return start, end
}

func closureDeclarationName(source []byte, start, end int) (int, int, bool) {
	index := start
	for index < end {
		for index < end && (source[index] == ' ' || source[index] == '\t' || source[index] == '\r' || source[index] == '\n') {
			index++
		}
		if index+1 < end && source[index] == '/' && source[index+1] == '/' {
			index += 2
			for index < end && source[index] != '\n' {
				index++
			}
			continue
		}
		if index+1 < end && source[index] == '/' && source[index+1] == '*' {
			index += 2
			for index+1 < end && !(source[index] == '*' && source[index+1] == '/') {
				index++
			}
			index += 2
			continue
		}
		break
	}
	finish := index
	for finish < end && ((source[finish] >= 'A' && source[finish] <= 'Z') ||
		(source[finish] >= 'a' && source[finish] <= 'z') ||
		(source[finish] >= '0' && source[finish] <= '9') || source[finish] == '_' || source[finish] == '$') {
		finish++
	}
	return index, finish, finish > index
}

func closureMatchingBrace(source []byte, open int, openChar, closeChar byte) int {
	depth := 0
	for index := open; index < len(source); index++ {
		switch source[index] {
		case openChar:
			depth++
		case closeChar:
			depth--
			if depth == 0 {
				return index
			}
		}
	}
	return -1
}

func closureStatementEnd(source []byte, start int) int {
	paren, bracket, brace, angle := 0, 0, 0, 0
	for index := start; index < len(source); index++ {
		switch source[index] {
		case '(':
			paren++
		case ')':
			paren--
		case '[':
			bracket++
		case ']':
			bracket--
		case '{':
			brace++
		case '}':
			brace--
		case '<':
			angle++
		case '>':
			if angle > 0 {
				angle--
			}
		case ';':
			if paren == 0 && bracket == 0 && brace == 0 && angle == 0 {
				return index
			}
		}
	}
	return -1
}

func closureSplitArguments(source []byte, start, end int) []closureByteSpan {
	spans := make([]closureByteSpan, 0)
	itemStart := start
	paren, bracket, brace, angle := 0, 0, 0, 0
	quote := byte(0)
	escaped, lineComment, blockComment := false, false, false
	for index := start; index < end; index++ {
		character := source[index]
		if lineComment {
			if character == '\n' {
				lineComment = false
			}
			continue
		}
		if blockComment {
			if character == '*' && index+1 < end && source[index+1] == '/' {
				blockComment = false
				index++
			}
			continue
		}
		if quote != 0 {
			if escaped {
				escaped = false
			} else if character == '\\' {
				escaped = true
			} else if character == quote {
				quote = 0
			}
			continue
		}
		if character == '/' && index+1 < end {
			if source[index+1] == '/' {
				lineComment = true
				index++
				continue
			}
			if source[index+1] == '*' {
				blockComment = true
				index++
				continue
			}
		}
		if character == '\'' || character == '"' || character == '`' {
			quote = character
			continue
		}
		switch character {
		case '(':
			paren++
		case ')':
			paren--
		case '[':
			bracket++
		case ']':
			bracket--
		case '{':
			brace++
		case '}':
			brace--
		case '<':
			angle++
		case '>':
			if angle > 0 {
				angle--
			}
		case ',':
			if paren == 0 && bracket == 0 && brace == 0 && angle == 0 {
				trimmedStart, trimmedEnd := closureTrimByteSpan(source, itemStart, index)
				if trimmedStart < trimmedEnd {
					spans = append(spans, closureByteSpan{start: itemStart, end: index})
				}
				itemStart = index + 1
			}
		}
	}
	trimmedStart, trimmedEnd := closureTrimByteSpan(source, itemStart, end)
	if trimmedStart < trimmedEnd {
		spans = append(spans, closureByteSpan{start: itemStart, end: end})
	}
	return spans
}
