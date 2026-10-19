#include "tree_sitter/array.h"
#include "tree_sitter/parser.h"

#include <string.h>
#include <wctype.h>

// Mostly a copy paste of tree-sitter-javascript/src/scanner.c

enum TokenType {
  AUTOMATIC_SEMICOLON,
  MULTILINE_COMMENT,
  STRING_START,
  STRING_END,
  STRING_CONTENT,
  PRIMARY_CONSTRUCTOR_KEYWORD,
  IMPORT_DOT,
  INTERPOLATION_EXPRESSION_START,
  INTERPOLATION_IDENTIFIER_START,
  BY_DELEGATION_HINT,
  FUNCTION_TYPE_ARROW_AHEAD,
  WHERE_HINT,
};

/* Pretty much all of this code is taken from the Julia tree-sitter
   parser.

   Julia has similar problems with multiline comments that can be nested,
   line comments, as well as line and multiline strings.

   The most heavily edited section is `scan_string_content`,
   particularly with respect to interpolation.
 */

// Block comments are easy to parse, but strings require extra-attention.

// The main problems that arise when parsing strings are:
// 1. Triple quoted strings allow single quotes inside. e.g. """ "foo" """.
// 2. Non-standard string literals don't allow interpolations or escape
//    sequences, but you can always write \" and \`.

// To efficiently store a delimiter, we take advantage of the fact that:
// (int)'"' == 34 && (34 & 1) == 0
// i.e. " has an even numeric representation, so we can store a triple
// quoted delimiter as (delimiter + 1).

#define DELIMITER_LENGTH 3

typedef char Delimiter;

// We use a stack to keep track of the string delimiters.
// Each entry is two bytes: [delimiter_byte, prefix_len_byte].
// delimiter_byte: '"' for single-quoted, '"'+1 for triple-quoted.
// prefix_len_byte: number of '$' signs required to trigger interpolation
//   (1 for regular strings and $"...", 2 for $$"...", etc.; max 255).
typedef Array(Delimiter) Stack;

static inline void stack_push(Stack *stack, char chr, bool triple, uint8_t prefix_len) {
  if (stack->size + 1 >= TREE_SITTER_SERIALIZATION_BUFFER_SIZE) abort();
  array_push(stack, (Delimiter)(triple ? (chr + 1) : chr));
  array_push(stack, (Delimiter)prefix_len);
}

static inline void stack_pop(Stack *stack) {
  if (stack->size < 2) abort();
  stack->size -= 2;
}

static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }

// Scanner functions

static bool scan_string_start(TSLexer *lexer, Stack *stack) {
  // Count leading '$' signs (the interpolation prefix). Capped at 255.
  uint8_t prefix_len = 0;
  while (lexer->lookahead == '$') {
    advance(lexer);
    if (prefix_len < 255) prefix_len++;
  }
  // Regular strings with no prefix still use a single '$' as the trigger.
  if (prefix_len == 0) prefix_len = 1;

  if (lexer->lookahead != '"') return false;
  advance(lexer);
  lexer->mark_end(lexer);
  for (unsigned count = 1; count < DELIMITER_LENGTH; ++count) {
    if (lexer->lookahead != '"') {
      // It's not a triple quoted delimiter.
      stack_push(stack, '"', false, prefix_len);
      return true;
    }
    advance(lexer);
  }
  lexer->mark_end(lexer);
  stack_push(stack, '"', true, prefix_len);
  return true;
}

static bool scan_string_content(TSLexer *lexer, Stack *stack,
                                const bool *valid_symbols) {
  if (stack->size < 2) return false;  // Stack is empty. We're not in a string.
  uint8_t prefix_len = (uint8_t)stack->contents[stack->size - 1];
  Delimiter raw_delim = stack->contents[stack->size - 2];
  bool is_triple = (raw_delim & 1) != 0;
  char end_char = is_triple ? (char)(raw_delim - 1) : (char)raw_delim;
  bool has_content = false;
  while (true) {
    if (lexer->lookahead == '\0') {
      // Stop at real end-of-input, but treat a literal NUL byte (not EOF) as
      // ordinary string content: consume it to guarantee forward progress.
      // Returning here without advancing would leave the lexer stuck at the
      // same offset (mirrors the NUL handling in scan_multiline_comment).
      if (lexer->eof(lexer)) break;
      advance(lexer);
      has_content = true;
      continue;
    }
    if (lexer->lookahead == '$') {
      // If we already have content, stop here so the caller can emit it
      // before we deal with the potential interpolation.
      if (has_content) {
        lexer->result_symbol = STRING_CONTENT;
        return true;
      }
      // Kotlin 2.1 multi-dollar interpolation: in a string with prefix_len N,
      // exactly N consecutive '$' followed by alpha/'{' triggers interpolation.
      // Excess leading '$' signs are literal string content.
      //
      // Strategy: consume the first '$' and mark_end there, then count
      // remaining '$' signs. If total > prefix_len, return STRING_CONTENT
      // for just the first '$' (tree-sitter rewinds to mark_end). On the
      // next scan call, the remaining dollars will be re-examined.
      advance(lexer);
      lexer->mark_end(lexer);
      uint16_t additional_dollars = 0;
      while (lexer->lookahead == '$') {
        advance(lexer);
        additional_dollars++;
      }
      uint16_t total_dollars = 1 + additional_dollars;
      if (total_dollars >= prefix_len &&
          (iswalpha(lexer->lookahead) || lexer->lookahead == '_' || lexer->lookahead == '{')) {
        if (total_dollars > prefix_len) {
          // Excess: emit first '$' as literal STRING_CONTENT.
          // mark_end is after the first '$'; tree-sitter rewinds there.
          lexer->result_symbol = STRING_CONTENT;
          return true;
        }
        // Exact match: emit interpolation start token.
        if (additional_dollars > 0) {
          lexer->mark_end(lexer);
        }
        if (valid_symbols[INTERPOLATION_EXPRESSION_START] &&
            lexer->lookahead == '{') {
          advance(lexer);
          // Empty interpolation "${}" is invalid Kotlin (compile error:
          // "Expecting an expression"). Refuse to emit the interpolation
          // token so the parser produces an ERROR node instead of matching
          // a zero-width expression.
          if (lexer->lookahead == '}') {
            return false;
          }
          lexer->mark_end(lexer);
          lexer->result_symbol = INTERPOLATION_EXPRESSION_START;
          return true;
        }
        if (valid_symbols[INTERPOLATION_IDENTIFIER_START] &&
            (iswalpha(lexer->lookahead) || lexer->lookahead == '_')) {
          lexer->result_symbol = INTERPOLATION_IDENTIFIER_START;
          return true;
        }
        return false;
      }
      // Not enough '$' signs or not followed by alpha/'{':
      // all consumed dollars are literal string content.
      if (additional_dollars > 0) {
        lexer->mark_end(lexer);
      }
      lexer->result_symbol = STRING_CONTENT;
      return true;
    }
    if (lexer->lookahead == '\\') {
      // if we see a \, then this might possibly escape a dollar sign
      // in which case, we should not defer to the interpolation
      advance(lexer);
      // this dollar sign is escaped, so it must be content.
      // we consume it here so we don't enter the dollar sign case above,
      // which leaves the possibility that it is an interpolation 
      if (lexer->lookahead == '$') {
        advance(lexer);
        // however this leaves an edgecase where an escaped dollar sign could
        // appear at the end of a single-quoted string (e.g "aa\$") which isn't handled
        // correctly; if we were at the end of the string, terminate properly
        if (!is_triple && lexer->lookahead == end_char) {
          stack_pop(stack);
          advance(lexer);
          lexer->mark_end(lexer);
          lexer->result_symbol = STRING_END;
          return true;
        }
        has_content = true;
        continue;
      } else if (is_triple && lexer->lookahead == end_char) {
        // In triple-quoted strings, `\` is NOT an escape character. So `\"` is
        // also literal backslash + quote, and the `"` might be the start of
        // the closing `"""`. Don't advance past it (at the end of the while
        // loop). Let the next iteration handle it.
        has_content = true;
        continue;
      }
    } else if (lexer->lookahead == end_char) {
      if (is_triple) {
        lexer->mark_end(lexer);
        for (unsigned count = 1; count < DELIMITER_LENGTH; ++count) {
          advance(lexer);
          if (lexer->lookahead != end_char) {
            lexer->mark_end(lexer);
            lexer->result_symbol = STRING_CONTENT;
            return true;
          }
        }

        /* This is so if we lex something like
           """foo"""
              ^
           where we are at the `f`, we should quit after
           reading `foo`, and ascribe it to STRING_CONTENT.

           Then, we restart and try to read the end.
           This is to prevent `foo` from being absorbed into
           the STRING_END token.
         */
        if (has_content && lexer->lookahead == end_char) {
          lexer->result_symbol = STRING_CONTENT;
          return true;
        }

        /* Since the string internals are all hidden in the syntax
           tree anyways, there's no point in going to the effort of
           specifically separating the string end from string contents.
           If we see a bunch of quotes in a row, then we just go until
           they stop appearing, then stop lexing and call it the
           string's end.
         */
        lexer->result_symbol = STRING_END;
        lexer->mark_end(lexer);
        while (lexer->lookahead == end_char) {
          advance(lexer);
          lexer->mark_end(lexer);
        }
        stack_pop(stack);
        return true;
      }
      if (has_content) {
        lexer->mark_end(lexer);
        lexer->result_symbol = STRING_CONTENT;
        return true;
      }
      stack_pop(stack);
      advance(lexer);
      lexer->mark_end(lexer);
      lexer->result_symbol = STRING_END;
      return true;
    }
    advance(lexer);
    has_content = true;
  }
  return false;
}


static bool scan_multiline_comment(TSLexer *lexer) {
  if (lexer->lookahead != '/') return false;
  advance(lexer);
  if (lexer->lookahead != '*') return false;
  advance(lexer);

  bool after_star = false;
  unsigned nesting_depth = 1;
  for (;;) {
    switch (lexer->lookahead) {
      case '*':
        advance(lexer);
        after_star = true;
        break;
      case '/':
        advance(lexer);
        if (after_star) {
          after_star = false;
          nesting_depth -= 1;
          if (nesting_depth == 0) {
            lexer->result_symbol = MULTILINE_COMMENT;
            lexer->mark_end(lexer);
            return true;
          }
        } else {
          after_star = false;
          if (lexer->lookahead == '*') {
            nesting_depth += 1;
            advance(lexer);
          }
        }
        break;
      case '\0':
        // Accept unterminated block comments at EOF rather than rejecting them.
        // This matches JetBrains PSI behavior which recognizes unclosed /* as a
        // BLOCK_COMMENT token (plus an error element). Without this, the scanner
        // returns false and tree-sitter tries to parse the comment delimiters
        // as operators/expressions.
        if (lexer->eof(lexer)) {
          lexer->result_symbol = MULTILINE_COMMENT;
          lexer->mark_end(lexer);
          return true;
        }
        // A literal NUL byte inside the comment (lookahead is '\0' but not EOF)
        // must be consumed like any other comment content. Returning here without
        // advancing leaves the lexer at the same offset, so tree-sitter re-invokes
        // the scanner forever on inputs such as "/*\0" -> a parse-time hang (DoS).
        // fallthrough
      default:
        advance(lexer);
        after_star = false;
        break;
    }
  }
}

static bool scan_whitespace_and_comments(TSLexer *lexer) {
  while (iswspace(lexer->lookahead)) skip(lexer);
  return true;
}

// Test for any identifier character other than the first character.
// This is meant to match the regexp [\p{L}_\p{Nd}]
// as found in '_alpha_identifier' (see grammar.js).
static bool is_word_char(int32_t c) {
  return (iswalnum(c) || c == '_');
}

// Scan for [the end of] a nonempty alphanumeric identifier or
// alphanumeric keyword (including '_').
static bool scan_for_word(TSLexer *lexer, const char* word, unsigned len) {
    skip(lexer);
    for (unsigned i = 0; i < len; ++i) {
      if (lexer->lookahead != word[i]) return false;
      skip(lexer);
    }
    // check that the identifier stops here
    if (is_word_char(lexer->lookahead)) return false;
    return true;
}

// Check if a sequence of characters matches the given word and is followed
// by a non-word character. Uses skip() so characters are not included in
// the current token.
static bool check_word(TSLexer *lexer, const char *word, unsigned len) {
  for (unsigned i = 0; i < len; i++) {
    if (lexer->lookahead != word[i]) return false;
    skip(lexer);
  }
  return !is_word_char(lexer->lookahead);
}

// Skip whitespace (space, tab, newline, CR) and comments (// and nested /* */)
// using skip() so characters are not included in the current token.
// Returns false if a bare '/' is encountered (not a comment), true otherwise.
static bool skip_whitespace_and_comments(TSLexer *lexer) {
  for (;;) {
    while (iswspace(lexer->lookahead)) skip(lexer);
    if (lexer->lookahead != '/') return true;
    skip(lexer);
    if (lexer->lookahead == '/') {
      // Line comment — skip to end of line
      skip(lexer);
      while (lexer->lookahead != '\n' && lexer->lookahead != '\r' &&
             !lexer->eof(lexer)) {
        skip(lexer);
      }
    } else if (lexer->lookahead == '*') {
      // Block comment — skip to */ (with nesting)
      skip(lexer);
      unsigned depth = 1;
      while (depth > 0 && !lexer->eof(lexer)) {
        if (lexer->lookahead == '*') {
          skip(lexer);
          if (lexer->lookahead == '/') { skip(lexer); depth--; }
        } else if (lexer->lookahead == '/') {
          skip(lexer);
          if (lexer->lookahead == '*') { skip(lexer); depth++; }
        } else {
          skip(lexer);
        }
      }
    } else {
      // Bare '/' — not a comment
      return false;
    }
  }
}

// After scan_for_word has matched "else", peek past optional whitespace
// and comments for "->". If found, this is a when-entry's `else ->`,
// not an if-else. Uses skip() so characters are not included in the
// current token.
static bool followed_by_arrow(TSLexer *lexer) {
  if (!skip_whitespace_and_comments(lexer)) return false;
  if (lexer->lookahead != '-') return false;
  skip(lexer);
  return lexer->lookahead == '>';
}

static bool is_constructor_modifier(const char *word) {
  return strcmp(word, "public") == 0 || strcmp(word, "private") == 0 ||
         strcmp(word, "protected") == 0 || strcmp(word, "internal") == 0 ||
         strcmp(word, "actual") == 0 || strcmp(word, "expect") == 0;
}

// Check if the current position has modifiers (public, private, protected,
// internal, actual, expect) followed by horizontal whitespace and "constructor".
// Uses skip() — safe to call speculatively since no token boundary is changed.
static bool check_modifier_then_constructor(TSLexer *lexer) {
  for (;;) {
    char word[20];
    unsigned len = 0;
    while (is_word_char(lexer->lookahead) && len < 19) {
      word[len++] = (char)lexer->lookahead;
      skip(lexer);
    }
    word[len] = '\0';

    if (!is_constructor_modifier(word)) {
      return false;
    }

    // Skip horizontal whitespace (not newlines)
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);

    if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
      return true;
    }
    if (!is_word_char(lexer->lookahead)) {
      return false;
    }
  }
}

// Look ahead past one or more annotations (e.g. @Bar, @com.example.Bar,
// @Bar(x=1)) and optional visibility modifier, then check for 'constructor'.
// All characters are consumed with skip() so nothing affects token boundaries.
static bool check_annotation_then_constructor(TSLexer *lexer) {
  // Skip one or more '@annotation' sequences
  while (lexer->lookahead == '@') {
    skip(lexer); // skip '@'
    if (!is_word_char(lexer->lookahead)) return false;
    // Read annotation name, including dot-separated qualifiers
    // (e.g. com.example.Inject)
    while (is_word_char(lexer->lookahead)) skip(lexer);
    while (lexer->lookahead == '.') {
      skip(lexer); // skip '.'
      if (!is_word_char(lexer->lookahead)) break;
      while (is_word_char(lexer->lookahead)) skip(lexer);
    }
    // Skip optional '(...)' argument list (handle nested parens and strings)
    if (lexer->lookahead == '(') {
      unsigned depth = 1;
      skip(lexer);
      while (depth > 0 && lexer->lookahead != '\0' && !lexer->eof(lexer)) {
        if (lexer->lookahead == '"') {
          // Skip over string literal to avoid miscounting parens inside strings
          skip(lexer);
          while (lexer->lookahead != '"' && lexer->lookahead != '\0' && !lexer->eof(lexer)) {
            if (lexer->lookahead == '\\') skip(lexer); // skip escaped char
            skip(lexer);
          }
          if (lexer->lookahead == '"') skip(lexer); // skip closing quote
        } else {
          if (lexer->lookahead == '(') depth++;
          else if (lexer->lookahead == ')') depth--;
          skip(lexer);
        }
      }
    }
    // Skip whitespace and newlines between annotations or before constructor
    while (iswspace(lexer->lookahead)) skip(lexer);
  }
  // Allow an optional visibility modifier before 'constructor'
  if (is_word_char(lexer->lookahead) && lexer->lookahead != 'c') {
    return check_modifier_then_constructor(lexer);
  }
  // Check directly for 'constructor'
  return check_word(lexer, "constructor", 11);
}

// Check if '[' or '(' after a newline is the start of a destructuring declaration
// (e.g. `[val x, val y] = d` or `(val name, val age) = person`) rather than
// an indexing expression or function call.
static bool lambda_has_arrow_ahead(TSLexer *lexer) {
  if (lexer->lookahead != '{') return false;
  skip(lexer);
  int max_chars = 128;
  int depth = 0;
  while (!lexer->eof(lexer) && max_chars-- > 0) {
    if (lexer->lookahead == '(') {
      depth++;
    } else if (lexer->lookahead == ')') {
      depth--;
    } else if (depth == 0 && lexer->lookahead == '-') {
      skip(lexer);
      if (lexer->lookahead == '>') return true;
    } else if (depth == 0 && (lexer->lookahead == '{' || lexer->lookahead == '}' || lexer->lookahead == ';')) {
      return false;
    }
    skip(lexer);
  }
  return false;
}

static bool paren_has_comma_ahead(TSLexer *lexer) {
  if (lexer->lookahead != '(') return false;
  skip(lexer);
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\n' || lexer->lookahead == '\r') {
    skip(lexer);
  }
  if (lexer->lookahead == 'v') {
    skip(lexer);
    if (lexer->lookahead == 'a') {
      skip(lexer);
      if (lexer->lookahead == 'r' || lexer->lookahead == 'l') {
        skip(lexer);
        if (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\n' || lexer->lookahead == '\r') {
          return false;
        }
      }
    }
  }
  int depth = 1;
  int max_chars = 256;
  bool has_comma = false;
  while (!lexer->eof(lexer) && max_chars-- > 0 && depth > 0) {
    if (lexer->lookahead == '(' || lexer->lookahead == '[' || lexer->lookahead == '{') {
      depth++;
    } else if (lexer->lookahead == ')' || lexer->lookahead == ']' || lexer->lookahead == '}') {
      depth--;
    } else if (depth == 1 && lexer->lookahead == ',') {
      has_comma = true;
    } else if (depth == 1 && lexer->lookahead == ';') {
      return false;
    }
    skip(lexer);
  }
  if (!has_comma) return false;
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\n' || lexer->lookahead == '\r') {
    skip(lexer);
  }
  if (lexer->lookahead == '=') {
    return false;
  }
  return true;
}

static bool scan_automatic_semicolon(TSLexer *lexer, const bool *valid_symbols) {
  lexer->result_symbol = AUTOMATIC_SEMICOLON;
  lexer->mark_end(lexer);

  bool sameline = true;
  for (;;) {
    if (lexer->eof(lexer)) return true;

    if (lexer->lookahead == ';') {
      advance(lexer);
      lexer->mark_end(lexer);
      return true;
    }

    if (!iswspace(lexer->lookahead)) break;

    if (lexer->lookahead == '\n') {
      skip(lexer);
      sameline = false;
      break;
    }

    if (lexer->lookahead == '\r') {
      skip(lexer);

      if (lexer->lookahead == '\n') skip(lexer);

      sameline = false;
      break;
    }

    skip(lexer);
  }

  // Skip whitespace and comments
  if (!scan_whitespace_and_comments(lexer))
    return false;

  if (sameline) {
    switch (lexer->lookahead) {
      // Insert imaginary semicolon before an 'import' but not in front
      // of other words or keywords starting with 'i'
      case 'i':
        return scan_for_word(lexer, "mport", 5);

      case ';':
        advance(lexer);
        lexer->mark_end(lexer);
        return true;

      // Don't insert a semicolon in other cases
      default:
        return false;
    }
  }

  switch (lexer->lookahead) {
      case ',':
      case '.':
      case ':':
      case '*':
      case '%':
      case '>':
      case '<':
      case '=':
      case '?':
      case '|':
      case '&':
        return false;
      case '{':
        return lambda_has_arrow_ahead(lexer);
      case '[':
        return true;
      case '(':
        if (paren_has_comma_ahead(lexer)) return false;
        return true;

      // Handle `/` — could be division, line comment, or block comment.
      // For division: no ASI (continuation operator).
      // For line comments (`//`): skip the comment(s) and check the next
      // real token. If continuation, suppress ASI (return false — tree-sitter
      // resets, parses line_comment internally, then re-checks ASI).
      // If non-continuation, insert ASI (return true at original mark_end).
      // For block comments (`/*`): advance through the comment and produce
      // MULTILINE_COMMENT. The parser then re-calls the scanner for the ASI
      // decision on whatever token follows the comment.
      case '/': {
        advance(lexer);
        if (lexer->lookahead == '/') {
          // Line comment
          skip(lexer);
          while (lexer->lookahead != '\n' && lexer->lookahead != '\r' &&
                 lexer->lookahead != 0 && !lexer->eof(lexer)) {
            skip(lexer);
          }
          if (!skip_whitespace_and_comments(lexer)) return false;
          switch (lexer->lookahead) {
            case '.': case ',': case ':': case '*': case '%':
            case '>': case '<': case '=':
            case '?': case '|': case '&': case '/':
            case ';':
              return false;
            case '{':
              return false;
            case '[':
              return true;
            case '(':
              return true;
            case '!':
              skip(lexer);
              if (lexer->lookahead == '=') return false;
              return true;
            case 'e': {
              char word[20];
              unsigned len = 0;
              while (is_word_char(lexer->lookahead) && len < 19) {
                word[len++] = (char)lexer->lookahead;
                skip(lexer);
              }
              word[len] = '\0';
              if (strcmp(word, "else") == 0) {
                if (followed_by_arrow(lexer)) return true;
                return false;
              }
              if (strcmp(word, "expect") == 0 && valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT]) {
                while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
                if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
                  return false;
                }
                if (is_word_char(lexer->lookahead) && check_modifier_then_constructor(lexer)) {
                  return false;
                }
              }
              return true;
            }
            case 'a': {
              char word[20];
              unsigned len = 0;
              while (is_word_char(lexer->lookahead) && len < 19) {
                word[len++] = (char)lexer->lookahead;
                skip(lexer);
              }
              word[len] = '\0';
              if (strcmp(word, "as") == 0) {
                return false;
              }
              if (strcmp(word, "actual") == 0 && valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT]) {
                while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
                if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
                  return false;
                }
                if (is_word_char(lexer->lookahead) && check_modifier_then_constructor(lexer)) {
                  return false;
                }
              }
              return true;
            }
            case 'w':
              if (valid_symbols[WHERE_HINT] && scan_for_word(lexer, "here", 4)) return false;
              return true;
            case 'c':
              if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT]) {
                if (check_word(lexer, "constructor", 11)) {
                  return false;
                }
              }
              if (scan_for_word(lexer, "atch", 4)) {
                while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
                if (lexer->lookahead == '(') return false;
                return true;
              }
              return true;
            case '@':
              if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT] &&
                  check_annotation_then_constructor(lexer)) {
                return false;
              }
              return true;
            case 'i':
              if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT]) {
                if (check_word(lexer, "nternal", 7)) {
                  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
                  if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
                    return false;
                  }
                  if (is_word_char(lexer->lookahead) && check_modifier_then_constructor(lexer)) {
                    return false;
                  }
                }
              }
              return true;
            case 'p':
              if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
                  !valid_symbols[STRING_CONTENT] &&
                  check_modifier_then_constructor(lexer)) {
                return false;
              }
              return true;
            case 'b':
              if (valid_symbols[BY_DELEGATION_HINT] &&
                  scan_for_word(lexer, "y", 1)) return false;
              return true;
            case 'f':
              if (scan_for_word(lexer, "inally", 6)) return false;
              return true;
            default:
              return true;
          }
        } else if (lexer->lookahead == '*') {
          // Block comment after newline — emit it directly so the comment
          // node gets its exact span, then let tree-sitter continue lexing.
          advance(lexer);
          unsigned nesting_depth = 1;
          bool after_star = false;
          while (nesting_depth > 0 && !lexer->eof(lexer)) {
            switch (lexer->lookahead) {
              case '*':
                advance(lexer);
                after_star = true;
                break;
              case '/':
                advance(lexer);
                if (after_star) {
                  after_star = false;
                  nesting_depth--;
                } else {
                  if (lexer->lookahead == '*') {
                    nesting_depth++;
                    advance(lexer);
                  }
                  after_star = false;
                }
                break;
              case '\0':
                if (lexer->eof(lexer)) {
                  lexer->result_symbol = MULTILINE_COMMENT;
                  lexer->mark_end(lexer);
                  return true;
                }
                // fallthrough
              default:
                advance(lexer);
                after_star = false;
                break;
            }
          }
          lexer->result_symbol = MULTILINE_COMMENT;
          lexer->mark_end(lexer);
          return true;
        }
        return false;
      }

      // In Kotlin, `+` and `-` after a newline are always prefix operators,
      // not binary continuation. If a binary operation is intended, the
      // operator must be placed at the end of the previous line:
      //   a +       // binary: a + b
      //     b
      //   a         // prefix: a; +b
      //   + b
      // The grammar ensures AUTOMATIC_SEMICOLON is only valid where a
      // statement could end, so this won't fire inside () or [] where
      // newlines don't terminate statements.
      case '+':
        return true;
      case '-':
        skip(lexer);
        if (lexer->lookahead == '>') return false;
        return true;

      // Don't insert a semicolon before `!=`, but do insert one before a unary `!`.
      case '!':
        skip(lexer);
        return lexer->lookahead != '=';

      // Don't insert a semicolon before 'by' in delegation contexts.
      // Gated on BY_DELEGATION_HINT so `by` remains a usable soft-keyword
      // identifier in non-delegation positions.
      case 'b':
        return !(valid_symbols[BY_DELEGATION_HINT] &&
                 scan_for_word(lexer, "y", 1));

      // Don't insert a semicolon before an else, unless it's
      // followed by "->" (a when-entry's else, not an if-else).
      // Also don't insert before 'expect constructor'.
      case 'e': {
        char word[20];
        unsigned len = 0;
        while (is_word_char(lexer->lookahead) && len < 19) {
          word[len++] = (char)lexer->lookahead;
          skip(lexer);
        }
        word[len] = '\0';
        if (strcmp(word, "else") == 0) {
          return followed_by_arrow(lexer);
        }
        if (strcmp(word, "expect") == 0 && valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT]) {
          while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
          if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
            return false;
          }
          if (is_word_char(lexer->lookahead) && check_modifier_then_constructor(lexer)) {
            return false;
          }
        }
        return true;
      }

      // Don't insert a semicolon before an as, or before 'actual constructor'
      case 'a': {
        char word[20];
        unsigned len = 0;
        while (is_word_char(lexer->lookahead) && len < 19) {
          word[len++] = (char)lexer->lookahead;
          skip(lexer);
        }
        word[len] = '\0';
        if (strcmp(word, "as") == 0) {
          return false;
        }
        if (strcmp(word, "actual") == 0 && valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT]) {
          while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
          if (lexer->lookahead == 'c' && check_word(lexer, "constructor", 11)) {
            return false;
          }
          if (is_word_char(lexer->lookahead) && check_modifier_then_constructor(lexer)) {
            return false;
          }
        }
        return true;
      }

      // Don't insert a semicolon before a where in type constraints context
      case 'w':
        return !(valid_symbols[WHERE_HINT] && scan_for_word(lexer, "here", 4));

      // Don't insert a semicolon before `instanceof`, or before `internal`
      // when followed by `constructor` in a class declaration context.
      case 'i':
        if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT] &&
            check_modifier_then_constructor(lexer)) {
          return false;
        }
        // Note: lexer has advanced past the word. For "instanceof", scan_for_word
        // can no longer match. But since "instanceof" is not a Kotlin keyword
        // (Kotlin uses "is"), this is acceptable — ASI is inserted, which is
        // the correct behavior for any non-constructor identifier.
        return true;

      // Don't insert a semicolon before `public/private/protected constructor`
      // in class declaration context.
      case 'p':
        if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT] &&
            check_modifier_then_constructor(lexer)) {
          return false;
        }
        return true;

      // Don't insert a semicolon before `constructor` if the parser expects
      // a primary constructor (class declaration context). In class body
      // context, PRIMARY_CONSTRUCTOR_KEYWORD won't be valid, so ASI is
      // inserted normally before secondary constructors.
      // Guard against error recovery mode where all symbols are valid.
      // Instead of suppressing ASI, we emit the constructor keyword directly
      // since it's an external token and the internal lexer won't match it.
      case 'c':
        if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT]) {
          const char *kw = "constructor";
          bool matched = true;
          for (unsigned i = 0; i < 11; i++) {
            if (lexer->lookahead != kw[i]) { matched = false; break; }
            advance(lexer);
          }
          if (matched && !is_word_char(lexer->lookahead)) {
            lexer->result_symbol = PRIMARY_CONSTRUCTOR_KEYWORD;
            lexer->mark_end(lexer);
            return true;
          }
          // If constructor didn't match, we've advanced past some chars.
          // Can't reliably check 'catch' now. Just insert ASI.
          return true;
        }
        // Not in constructor context — check for 'catch'
        if (scan_for_word(lexer, "atch", 4)) {
          while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
          if (lexer->lookahead == '(') return false;
          return true;
        }
        return true;

      // Don't insert a semicolon before finally (continues try_expression)
      case 'f':
        return !scan_for_word(lexer, "inally", 6);

      // Don't insert a semicolon before an annotation that precedes 'constructor'
      // e.g. `class Foo\n@Bar\nconstructor(...)` — the @Bar is a constructor modifier
      case '@':
        if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] &&
            !valid_symbols[STRING_CONTENT] &&
            check_annotation_then_constructor(lexer)) {
          return false;
        }
        return true;

      case ';':
        advance(lexer);
        lexer->mark_end(lexer);
        return true;

      default:
        return true;
  }
}


// Scan a dot in import identifiers. Matches '.' normally, but when the dot
// is followed by a newline and then the 'import' keyword, produces an
// AUTOMATIC_SEMICOLON (zero-width, before the dot) instead. This cleanly
// terminates the current import_header, preventing malformed imports
// (e.g. trailing dots) from bleeding into subsequent valid imports.
static bool scan_import_dot(TSLexer *lexer) {
  if (lexer->lookahead != '.') return false;

  // Mark end BEFORE consuming the dot — this is where ASI would go
  lexer->mark_end(lexer);

  advance(lexer);

  // Peek ahead: skip horizontal whitespace, check for newline
  bool found_newline = false;
  while (iswspace(lexer->lookahead)) {
    if (lexer->lookahead == '\n' || lexer->lookahead == '\r') {
      found_newline = true;
    }
    skip(lexer);
  }

  if (found_newline && lexer->lookahead == 'i' &&
      scan_for_word(lexer, "mport", 5)) {
    // Trailing dot followed by 'import' on next line — produce ASI
    // instead of the dot. mark_end was set before the dot, so the
    // semicolon is zero-width at that position.
    lexer->result_symbol = AUTOMATIC_SEMICOLON;
    return true;
  }

  // Normal dot — include it in the token
  lexer->result_symbol = IMPORT_DOT;
  lexer->mark_end(lexer);
  return true;
}

static inline bool is_ascii_space(int32_t c) {
  return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}

static bool scan_function_type_arrow_ahead(TSLexer *lexer, bool *consumed_paren) {
  *consumed_paren = false;
  bool has_whitespace = false;
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    has_whitespace = true;
    skip(lexer);
  }

  if (lexer->lookahead != '(') {
    return false;
  }

  // In Kotlin, whitespace is not allowed between an annotation and its arguments.
  // Any whitespace before '(' means it belongs to a parenthesized or function type.
  if (has_whitespace) {
    lexer->mark_end(lexer);
    return true;
  }

  lexer->mark_end(lexer);
  advance(lexer);
  *consumed_paren = true;

  while (is_ascii_space(lexer->lookahead)) {
    advance(lexer);
  }

  if (lexer->lookahead == ')') {
    advance(lexer);
    while (is_ascii_space(lexer->lookahead)) {
      advance(lexer);
    }
    if (lexer->lookahead == '-') {
      advance(lexer);
      if (lexer->lookahead == '>') {
        return true;
      }
    }
    return false;
  }

  char first_ch = (char)lexer->lookahead;
  if (first_ch == '"' || first_ch == '\'' || (first_ch >= '0' && first_ch <= '9') || first_ch == '=') {
    return false;
  }

  int depth = 1;
  int max_chars = 128;

  while (!lexer->eof(lexer) && max_chars-- > 0) {
    char ch = (char)lexer->lookahead;
    if (ch == '(') {
      depth++;
      advance(lexer);
    } else if (ch == ')') {
      depth--;
      advance(lexer);
      while (is_ascii_space(lexer->lookahead)) {
        advance(lexer);
      }
      if (lexer->lookahead == '-') {
        advance(lexer);
        if (lexer->lookahead == '>') {
          return true;
        }
      }
      if (depth == 0) {
        return false;
      }
    } else if (ch == '"' || ch == '\'') {
      return false;
    } else if (ch == ';' || ch == '{' || ch == '}') {
      return false;
    } else {
      advance(lexer);
    }
  }

  return false;
}

bool tree_sitter_kotlin_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  if (valid_symbols[FUNCTION_TYPE_ARROW_AHEAD]) {
    bool consumed_paren = false;
    if (scan_function_type_arrow_ahead(lexer, &consumed_paren)) {
      lexer->result_symbol = FUNCTION_TYPE_ARROW_AHEAD;
      return true;
    }
    if (consumed_paren) {
      return false;
    }
  }

  // BY_DELEGATION_HINT is declared in the grammar (optional, before `by` in
  // explicit_delegation and property_delegate) purely so it appears in
  // valid_symbols when the parser is in a delegation context. The scanner
  // never emits it; it's used only as a context flag in scan_automatic_semicolon.
  if (valid_symbols[AUTOMATIC_SEMICOLON]) {
    bool ret = scan_automatic_semicolon(lexer, valid_symbols);
    // if we fail to find an automatic semicolon, it's still possible that we may
    // want to lex a string or comment later
    if (ret) return ret;
  }

  // Match dots in import identifiers, refusing dots that would cause
  // malformed imports to bleed into subsequent import statements.
  if (valid_symbols[IMPORT_DOT]) {
    if (scan_import_dot(lexer)) return true;
  }

  // Match 'constructor' keyword for primary constructors when on the same line
  // (the cross-newline case is handled inside scan_automatic_semicolon)
  if (valid_symbols[PRIMARY_CONSTRUCTOR_KEYWORD] && !valid_symbols[STRING_CONTENT]) {
    while (iswspace(lexer->lookahead)) skip(lexer);
    if (lexer->lookahead == 'c') {
      const char *kw = "constructor";
      bool matched = true;
      for (unsigned i = 0; i < 11; i++) {
        if (lexer->lookahead != kw[i]) { matched = false; break; }
        advance(lexer);
      }
      if (matched && !is_word_char(lexer->lookahead)) {
        lexer->result_symbol = PRIMARY_CONSTRUCTOR_KEYWORD;
        lexer->mark_end(lexer);
        return true;
      }
    }
  }

  // content, end, or interpolation start
  if (valid_symbols[STRING_CONTENT] || valid_symbols[INTERPOLATION_EXPRESSION_START] ||
      valid_symbols[INTERPOLATION_IDENTIFIER_START]) {
    if (scan_string_content(lexer, payload, valid_symbols)) return true;
  }

  // a string might follow after some whitespace, so we can't lookahead
  // until we get rid of it
  while (iswspace(lexer->lookahead)) skip(lexer);

  if (valid_symbols[STRING_START] && scan_string_start(lexer, payload)) {
    lexer->result_symbol = STRING_START;
    return true;
  }

  if (valid_symbols[MULTILINE_COMMENT] && scan_multiline_comment(lexer)) {
    return true;
  }

  return false;
}

void *tree_sitter_kotlin_external_scanner_create() {
  Stack *stack = ts_calloc(1, sizeof(Stack));
  if (stack == NULL) abort();
  array_init(stack);
  return stack;
}

void tree_sitter_kotlin_external_scanner_destroy(void *payload) {
  Stack *stack = (Stack *)payload;
  array_delete(stack);
  ts_free(stack);
}

unsigned tree_sitter_kotlin_external_scanner_serialize(void *payload, char *buffer) {
  Stack *stack = (Stack *)payload;
  unsigned n = stack->size;
  if (n > TREE_SITTER_SERIALIZATION_BUFFER_SIZE) {
    n = TREE_SITTER_SERIALIZATION_BUFFER_SIZE;
  }
  if (n > 0) {
    // it's an undefined behavior to memcpy 0 bytes
    memcpy(buffer, stack->contents, n);
  }
  return n;
}

void tree_sitter_kotlin_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  Stack *stack = (Stack *)payload;
  // Stack entries are 2 bytes each (delimiter + prefix_len).
  // Discard corrupted state with odd length.
  if (length > 0 && length % 2 == 0) {
    array_reserve(stack, length);
    memcpy(stack->contents, buffer, length);
    stack->size = length;
  } else {
    array_clear(stack);
  }
}
