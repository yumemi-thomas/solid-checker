//! A user-written list of callee names, in upstream's grammar.
//!
//! eslint-plugin-solid's `createNameMatcher` (0.18.1, `src/utils.ts`) reads each
//! entry of a name list as one of:
//!
//! 1. `/pattern/` (longer than two characters, starting and ending with `/`):
//!    a regular expression, **unanchored**, so `/define/` also matches
//!    `redefine`. An entry that does not parse as a regular expression falls
//!    back to exact comparison of the whole entry;
//! 2. an entry containing `*`: the parts between the stars are literal and each
//!    star stands for any run of `[a-zA-Z0-9_$]`, the whole name anchored;
//! 3. anything else: exact equality.
//!
//! The regular-expression arm here is a subset of JavaScript's, matched by a
//! Thompson simulation (no backtracking, no catastrophic input): literals, `.`,
//! classes with ranges and the `\d \w \s` escapes, `^`, `$`, groups, `|` and the
//! quantifiers `* + ? {n} {n,} {n,m}`. Lookaround, backreferences, named groups,
//! `\b`, `\p`, `\u`, `\x` and a literal `{` do not parse, so an entry using one
//! compares exactly, as an invalid regular expression does upstream. That can
//! only withhold a report the JavaScript pattern would make, never invent one.

use crate::HashSet;

#[derive(Clone, Debug)]
pub(crate) struct NameMatcher {
    patterns: Vec<Pattern>,
}

#[derive(Clone, Debug)]
enum Pattern {
    Exact(String),
    Wildcard(Vec<String>),
    Regex(Regex),
}

impl NameMatcher {
    pub(crate) fn new(entries: &[String]) -> Self {
        let patterns = entries
            .iter()
            .map(|entry| {
                let length = entry.encode_utf16().count();
                if length > 2
                    && entry.starts_with('/')
                    && entry.ends_with('/')
                    && let Some(regex) = Regex::parse(&entry[1..entry.len() - 1])
                {
                    return Pattern::Regex(regex);
                }
                if length > 2 && entry.starts_with('/') && entry.ends_with('/') {
                    return Pattern::Exact(entry.clone());
                }
                if entry.contains('*') {
                    return Pattern::Wildcard(entry.split('*').map(str::to_owned).collect());
                }
                Pattern::Exact(entry.clone())
            })
            .collect();
        Self { patterns }
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        self.patterns.iter().any(|pattern| match pattern {
            Pattern::Exact(exact) => exact == name,
            Pattern::Wildcard(parts) => wildcard_matches(parts, name),
            Pattern::Regex(regex) => regex.is_match(name),
        })
    }
}

fn is_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn gap_is_name_characters(gap: &str) -> bool {
    gap.chars().all(is_name_character)
}

/// `^part0[a-zA-Z0-9_$]*part1[a-zA-Z0-9_$]*…partN$`.
fn wildcard_matches(parts: &[String], name: &str) -> bool {
    let Some((first, rest)) = parts.split_first() else {
        return false;
    };
    let Some(after_first) = name.strip_prefix(first.as_str()) else {
        return false;
    };
    wildcard_tail(rest, after_first)
}

fn wildcard_tail(parts: &[String], remaining: &str) -> bool {
    match parts {
        [] => remaining.is_empty(),
        [last] => {
            remaining.ends_with(last.as_str())
                && gap_is_name_characters(&remaining[..remaining.len() - last.len()])
        }
        [part, rest @ ..] => remaining.match_indices(part.as_str()).any(|(index, _)| {
            gap_is_name_characters(&remaining[..index])
                && wildcard_tail(rest, &remaining[index + part.len()..])
        }),
    }
}

/// Entries beyond this many instructions do not compile (a `{n,m}` blow-up).
const INSTRUCTION_BUDGET: usize = 20_000;

#[derive(Clone, Debug)]
struct Regex {
    program: Vec<Inst>,
}

#[derive(Clone, Debug)]
enum Inst {
    Char(char),
    Any,
    Class { items: Vec<ClassItem>, negate: bool },
    Split(usize, usize),
    Jump(usize),
    AssertStart,
    AssertEnd,
    Match,
}

#[derive(Clone, Copy, Debug)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    Any,
    Class { items: Vec<ClassItem>, negate: bool },
    Start,
    End,
    Group(Vec<Vec<Node>>),
    Repeat(Box<Node>, usize, Option<usize>),
}

impl ClassItem {
    fn contains(self, character: char) -> bool {
        match self {
            Self::Range(low, high) => (low..=high).contains(&character),
            Self::Digit(positive) => character.is_ascii_digit() == positive,
            Self::Word(positive) => is_name_character_word(character) == positive,
            Self::Space(positive) => is_space(character) == positive,
        }
    }
}

fn is_name_character_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_space(character: char) -> bool {
    matches!(
        character,
        ' ' | '\t' | '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

struct Parser<'p> {
    input: &'p [char],
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Alternatives separated by `|`, up to the end of the input or a `)`.
    fn alternatives(&mut self, depth: usize) -> Option<Vec<Vec<Node>>> {
        if depth > 32 {
            return None;
        }
        let mut alternatives = vec![self.sequence(depth)?];
        while self.eat('|') {
            alternatives.push(self.sequence(depth)?);
        }
        Some(alternatives)
    }

    fn sequence(&mut self, depth: usize) -> Option<Vec<Node>> {
        let mut nodes = Vec::new();
        while let Some(character) = self.peek() {
            if character == '|' || character == ')' {
                break;
            }
            let atom = self.atom(depth)?;
            nodes.push(self.quantified(atom)?);
        }
        Some(nodes)
    }

    fn quantified(&mut self, atom: Node) -> Option<Node> {
        let (low, high) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                self.position += 1;
                let low = self.number()?;
                let high = if self.eat(',') {
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.number()?)
                    }
                } else {
                    Some(low)
                };
                if self.peek() != Some('}') || high.is_some_and(|high| high < low) {
                    return None;
                }
                (low, high)
            }
            _ => return Some(atom),
        };
        self.position += 1;
        // A lazy `?` changes which match is found, never whether one exists.
        self.eat('?');
        if matches!(atom, Node::Start | Node::End) || low > 1000 || high.is_some_and(|h| h > 1000) {
            return None;
        }
        Some(Node::Repeat(Box::new(atom), low, high))
    }

    fn number(&mut self) -> Option<usize> {
        let start = self.position;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.position += 1;
        }
        if start == self.position {
            return None;
        }
        self.input[start..self.position]
            .iter()
            .collect::<String>()
            .parse()
            .ok()
    }

    fn atom(&mut self, depth: usize) -> Option<Node> {
        let character = self.peek()?;
        self.position += 1;
        match character {
            '.' => Some(Node::Any),
            '^' => Some(Node::Start),
            '$' => Some(Node::End),
            '(' => {
                if self.eat('?') && !self.eat(':') {
                    return None;
                }
                let alternatives = self.alternatives(depth + 1)?;
                self.eat(')').then_some(Node::Group(alternatives))
            }
            '[' => self.class(),
            '\\' => match self.escape()? {
                Escape::Literal(character) => Some(Node::Char(character)),
                Escape::Class(item) => Some(Node::Class {
                    items: vec![item],
                    negate: false,
                }),
            },
            '*' | '+' | '?' | '{' | '}' => None,
            other => Some(Node::Char(other)),
        }
    }

    fn escape(&mut self) -> Option<Escape> {
        let character = self.peek()?;
        self.position += 1;
        Some(match character {
            'd' => Escape::Class(ClassItem::Digit(true)),
            'D' => Escape::Class(ClassItem::Digit(false)),
            'w' => Escape::Class(ClassItem::Word(true)),
            'W' => Escape::Class(ClassItem::Word(false)),
            's' => Escape::Class(ClassItem::Space(true)),
            'S' => Escape::Class(ClassItem::Space(false)),
            't' => Escape::Literal('\t'),
            'n' => Escape::Literal('\n'),
            'r' => Escape::Literal('\r'),
            'f' => Escape::Literal('\u{0c}'),
            'v' => Escape::Literal('\u{0b}'),
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '/' | '^'
            | '$' | '-' => Escape::Literal(character),
            _ => return None,
        })
    }

    fn class(&mut self) -> Option<Node> {
        let negate = self.eat('^');
        let mut items = Vec::new();
        loop {
            let character = self.peek()?;
            self.position += 1;
            if character == ']' {
                return Some(Node::Class { items, negate });
            }
            let low = match character {
                '\\' => match self.escape()? {
                    Escape::Class(item) => {
                        items.push(item);
                        continue;
                    }
                    Escape::Literal(literal) => literal,
                },
                other => other,
            };
            if self.peek() == Some('-') && self.input.get(self.position + 1) != Some(&']') {
                self.position += 1;
                let high = match self.peek()? {
                    '\\' => {
                        self.position += 1;
                        match self.escape()? {
                            Escape::Literal(literal) => literal,
                            Escape::Class(_) => return None,
                        }
                    }
                    other => {
                        self.position += 1;
                        other
                    }
                };
                if high < low {
                    return None;
                }
                items.push(ClassItem::Range(low, high));
            } else {
                items.push(ClassItem::Range(low, low));
            }
        }
    }
}

enum Escape {
    Literal(char),
    Class(ClassItem),
}

fn emit_sequence(nodes: &[Node], program: &mut Vec<Inst>) -> Option<()> {
    for node in nodes {
        emit(node, program)?;
        if program.len() > INSTRUCTION_BUDGET {
            return None;
        }
    }
    Some(())
}

fn emit_alternatives(alternatives: &[Vec<Node>], program: &mut Vec<Inst>) -> Option<()> {
    let Some((last, init)) = alternatives.split_last() else {
        return Some(());
    };
    let mut jumps = Vec::new();
    for alternative in init {
        let split = program.len();
        program.push(Inst::Split(split + 1, 0));
        emit_sequence(alternative, program)?;
        jumps.push(program.len());
        program.push(Inst::Jump(0));
        let next = program.len();
        program[split] = Inst::Split(split + 1, next);
    }
    emit_sequence(last, program)?;
    let end = program.len();
    for jump in jumps {
        program[jump] = Inst::Jump(end);
    }
    Some(())
}

fn emit(node: &Node, program: &mut Vec<Inst>) -> Option<()> {
    match node {
        Node::Char(character) => program.push(Inst::Char(*character)),
        Node::Any => program.push(Inst::Any),
        Node::Class { items, negate } => program.push(Inst::Class {
            items: items.clone(),
            negate: *negate,
        }),
        Node::Start => program.push(Inst::AssertStart),
        Node::End => program.push(Inst::AssertEnd),
        Node::Group(alternatives) => emit_alternatives(alternatives, program)?,
        Node::Repeat(inner, low, high) => {
            for _ in 0..*low {
                emit(inner, program)?;
            }
            match high {
                None => {
                    let split = program.len();
                    program.push(Inst::Split(split + 1, 0));
                    emit(inner, program)?;
                    program.push(Inst::Jump(split));
                    let end = program.len();
                    program[split] = Inst::Split(split + 1, end);
                }
                Some(high) => {
                    let mut splits = Vec::new();
                    for _ in *low..*high {
                        splits.push(program.len());
                        program.push(Inst::Split(0, 0));
                        emit(inner, program)?;
                    }
                    let end = program.len();
                    for split in splits {
                        program[split] = Inst::Split(split + 1, end);
                    }
                }
            }
        }
    }
    (program.len() <= INSTRUCTION_BUDGET).then_some(())
}

impl Regex {
    fn parse(pattern: &str) -> Option<Self> {
        let input = pattern.chars().collect::<Vec<_>>();
        let mut parser = Parser {
            input: &input,
            position: 0,
        };
        let alternatives = parser.alternatives(0)?;
        if parser.position != input.len() {
            return None;
        }
        let mut program = Vec::new();
        emit_alternatives(&alternatives, &mut program)?;
        program.push(Inst::Match);
        Some(Self { program })
    }

    /// Whether the pattern matches anywhere in `text` (JavaScript's `test`).
    fn is_match(&self, text: &str) -> bool {
        let characters = text.chars().collect::<Vec<_>>();
        let mut current = HashSet::default();
        let mut next = HashSet::default();
        for position in 0..=characters.len() {
            // Unanchored: a new thread may start at every position.
            if self.add(&mut current, 0, position, &characters) {
                return true;
            }
            let Some(&character) = characters.get(position) else {
                break;
            };
            next.clear();
            for &state in &current {
                let advances = match &self.program[state] {
                    Inst::Char(expected) => *expected == character,
                    Inst::Any => !matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'),
                    Inst::Class { items, negate } => {
                        items.iter().any(|item| item.contains(character)) != *negate
                    }
                    _ => false,
                };
                if advances && self.add(&mut next, state + 1, position + 1, &characters) {
                    return true;
                }
            }
            std::mem::swap(&mut current, &mut next);
        }
        false
    }

    /// Adds `state` and its epsilon closure at `position`; true on `Match`.
    fn add(
        &self,
        states: &mut HashSet<usize>,
        state: usize,
        position: usize,
        characters: &[char],
    ) -> bool {
        if !states.insert(state) {
            return false;
        }
        match &self.program[state] {
            Inst::Match => true,
            Inst::Jump(target) => self.add(states, *target, position, characters),
            Inst::Split(first, second) => {
                self.add(states, *first, position, characters)
                    || self.add(states, *second, position, characters)
            }
            Inst::AssertStart => position == 0 && self.add(states, state + 1, position, characters),
            Inst::AssertEnd => {
                position == characters.len() && self.add(states, state + 1, position, characters)
            }
            Inst::Char(_) | Inst::Any | Inst::Class { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NameMatcher;

    fn matcher(entries: &[&str]) -> NameMatcher {
        NameMatcher::new(&entries.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn exact_entries_compare_whole_names() {
        let m = matcher(&["defineLoader"]);
        assert!(m.matches("defineLoader"));
        assert!(!m.matches("defineLoaders"));
        assert!(!m.matches("redefineLoader"));
    }

    #[test]
    fn a_star_is_a_run_of_name_characters_and_the_name_is_anchored() {
        let m = matcher(&["define*"]);
        assert!(m.matches("define"));
        assert!(m.matches("defineLoader"));
        assert!(m.matches("define_x$1"));
        assert!(!m.matches("redefine"));
        assert!(!m.matches("define-x"));
        let middle = matcher(&["a*b*c"]);
        assert!(middle.matches("abc"));
        assert!(middle.matches("aXXbYYc"));
        assert!(!middle.matches("aXXbYY"));
        assert!(matcher(&["*"]).matches("anything"));
        assert!(matcher(&["*Loader"]).matches("userLoader"));
    }

    #[test]
    fn a_regex_entry_is_unanchored() {
        let m = matcher(&["/define/"]);
        assert!(m.matches("define"));
        assert!(m.matches("redefine"));
        assert!(m.matches("defineLoader"));
        assert!(!m.matches("defin"));
        let anchored = matcher(&["/^get[A-Z]\\w*$/"]);
        assert!(anchored.matches("getUser"));
        assert!(!anchored.matches("getuser"));
        assert!(!anchored.matches("xgetUser"));
        let alternation = matcher(&["/^(?:foo|ba+r){1,2}$/"]);
        assert!(alternation.matches("foobaar"));
        assert!(alternation.matches("bar"));
        assert!(!alternation.matches("foobarfoo"));
        assert!(matcher(&["/[a-c]x?/"]).matches("zzbzz"));
        assert!(!matcher(&["/[^a-z]/"]).matches("abc"));
    }

    #[test]
    fn an_entry_that_does_not_parse_compares_exactly() {
        // Unbalanced group, a nothing-to-repeat star, lookahead and `\b` are
        // not in the subset: each compares as the literal entry.
        for entry in ["/(a/", "/*a/", "/a(?=b)/", "/\\bfoo/"] {
            let m = matcher(&[entry]);
            assert!(m.matches(entry), "{entry}");
            assert!(!m.matches("a"), "{entry}");
            assert!(!m.matches("foo"), "{entry}");
        }
    }

    #[test]
    fn two_characters_between_slashes_is_not_a_regex() {
        assert!(matcher(&["//"]).matches("//"));
        assert!(!matcher(&["//"]).matches("x"));
        assert!(matcher(&["/a/"]).matches("a"));
    }

    #[test]
    fn an_empty_list_matches_nothing() {
        assert!(!matcher(&[]).matches("anything"));
    }
}
