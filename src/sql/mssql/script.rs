//! Scripts written for `sqlcmd -i`, run by the built-in client.
//!
//! sqlcmd is more than a pipe to the server: it cuts a script into batches at
//! `GO` lines, runs its own `:` commands (`:setvar`, `:r`, `:on error`, ...)
//! and `!!` shell escapes, and substitutes `$(name)` scripting variables unless
//! `-x` is given. The built-in client does only the first: every other feature
//! is refused with the line that uses it, never passed on to the server or
//! silently dropped. The scripts ibcmd-rs generates use none of them.

use anyhow::{Result, bail};

use crate::sql::ScriptVariables;

/// A complete immutable script admitted before dispatch. This first adapter
/// materializes UTF-8 and the existing splitter's batches; it is not an O(1)
/// streaming SQL executor. Late input errors cannot follow an early COMMIT.
pub struct SealedSqlScript {
    pub(super) batches: Vec<ScriptBatch>,
    sha256: [u8; 32],
    byte_length: usize,
}

impl SealedSqlScript {
    pub fn read(mut input: impl std::io::Read, variables: ScriptVariables) -> Result<Self> {
        use sha2::{Digest, Sha256};
        let mut bytes = Vec::new();
        let mut block = [0u8; 64 * 1024];
        loop {
            let count = match input.read(&mut block) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if count == 0 {
                break;
            }
            if count > block.len() {
                bail!("reader reported bytes outside its supplied buffer");
            }
            bytes
                .len()
                .checked_add(count)
                .ok_or_else(|| anyhow::anyhow!("SQL input extent overflow"))?;
            bytes.try_reserve(count)?;
            bytes.extend_from_slice(&block[..count]);
        }
        let sha256 = Sha256::digest(&bytes).into();
        let byte_length = bytes.len();
        let text = String::from_utf8(bytes)?;
        let batches = split_batches(&text, variables)?;
        Ok(Self {
            batches,
            sha256,
            byte_length,
        })
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub fn byte_length(&self) -> usize {
        self.byte_length
    }
    pub fn batches(&self) -> &[ScriptBatch] {
        &self.batches
    }
}

/// One batch of a script and the line it starts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBatch {
    pub first_line: usize,
    pub text: String,
}

/// Cuts a sqlcmd script into the batches sqlcmd would send, in order.
///
/// A batch ends at a line holding only `GO` (any case), optionally followed by
/// a repeat count (`GO 3` sends the batch three times). `GO`, `:` commands and
/// `!!` count only at the start of a line outside string literals, quoted
/// identifiers and comments. Batches with nothing but whitespace are dropped,
/// as sqlcmd drops them.
pub fn split_batches(script: &str, variables: ScriptVariables) -> Result<Vec<ScriptBatch>> {
    let script = script.strip_prefix('\u{feff}').unwrap_or(script);
    if variables == ScriptVariables::Refuse
        && let Some(offset) = script.find("$(")
    {
        let line = script[..offset].matches('\n').count() + 1;
        bail!(
            "script line {line} uses a sqlcmd scripting variable ($(...)); the built-in SQL \
             client does not substitute them (pass --sqlcmd to run sqlcmd.exe)"
        );
    }
    let mut batches = Vec::new();
    let mut current = String::new();
    let mut current_first_line = 1usize;
    let mut state = LexState::Code;
    for (index, line) in script.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        if state == LexState::Code {
            let content = line.trim();
            if let Some(count) = go_count(content, line_number)? {
                push_batch(&mut batches, &current, current_first_line, count);
                current.clear();
                current_first_line = line_number + 1;
                continue;
            }
            if let Some(command) = sqlcmd_command(content) {
                bail!(
                    "script line {line_number} is a sqlcmd command ({command}); the built-in SQL \
                     client runs T-SQL batches only (pass --sqlcmd to run sqlcmd.exe)"
                );
            }
        }
        if current.is_empty() {
            current_first_line = line_number;
        }
        state = scan_line(line, state);
        current.push_str(line);
    }
    push_batch(&mut batches, &current, current_first_line, 1);
    Ok(batches)
}

fn push_batch(batches: &mut Vec<ScriptBatch>, text: &str, first_line: usize, count: usize) {
    if text.trim().is_empty() {
        return;
    }
    for _ in 0..count {
        batches.push(ScriptBatch {
            first_line,
            text: text.to_owned(),
        });
    }
}

/// `GO` or `GO <count>` on a line of its own.
fn go_count(content: &str, line_number: usize) -> Result<Option<usize>> {
    let mut words = content.split_whitespace();
    let Some(first) = words.next() else {
        return Ok(None);
    };
    if !first.eq_ignore_ascii_case("go") {
        return Ok(None);
    }
    let count = match (words.next(), words.next()) {
        (None, _) => 1,
        (Some(count), None) => match count.parse::<usize>() {
            Ok(count) if count > 0 => count,
            _ => bail!("script line {line_number}: GO takes a positive repeat count"),
        },
        // `GO` followed by more words is T-SQL text, not a separator (sqlcmd
        // would refuse it; the server then reports the syntax).
        _ => return Ok(None),
    };
    Ok(Some(count))
}

/// The sqlcmd command a line starts with, if any.
fn sqlcmd_command(content: &str) -> Option<String> {
    if content.starts_with("!!") {
        return Some("!!".to_owned());
    }
    let rest = content.strip_prefix(':')?;
    let name = rest
        .split(|ch: char| ch.is_whitespace())
        .next()
        .unwrap_or_default();
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    Some(format!(":{name}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LexState {
    Code,
    /// Inside `'...'` (a string literal, `''` escapes a quote).
    String,
    /// Inside `[...]` (`]]` escapes a bracket).
    Bracket,
    /// Inside `"..."` (a quoted identifier or, with QUOTED_IDENTIFIER OFF, a
    /// string; `""` escapes a quote).
    DoubleQuote,
    /// Inside `/* ... */`; T-SQL block comments nest.
    Comment(usize),
}

/// Where a line leaves the lexer, so that `GO` inside a multi-line literal or
/// comment is not taken for a separator.
fn scan_line(line: &str, mut state: LexState) -> LexState {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        state = match state {
            LexState::Code => match (byte, next) {
                (b'-', Some(b'-')) => return LexState::Code,
                (b'/', Some(b'*')) => {
                    index += 1;
                    LexState::Comment(1)
                }
                (b'\'', _) => LexState::String,
                (b'[', _) => LexState::Bracket,
                (b'"', _) => LexState::DoubleQuote,
                _ => LexState::Code,
            },
            LexState::String => match (byte, next) {
                (b'\'', Some(b'\'')) => {
                    index += 1;
                    LexState::String
                }
                (b'\'', _) => LexState::Code,
                _ => LexState::String,
            },
            LexState::Bracket => match (byte, next) {
                (b']', Some(b']')) => {
                    index += 1;
                    LexState::Bracket
                }
                (b']', _) => LexState::Code,
                _ => LexState::Bracket,
            },
            LexState::DoubleQuote => match (byte, next) {
                (b'"', Some(b'"')) => {
                    index += 1;
                    LexState::DoubleQuote
                }
                (b'"', _) => LexState::Code,
                _ => LexState::DoubleQuote,
            },
            LexState::Comment(depth) => match (byte, next) {
                (b'/', Some(b'*')) => {
                    index += 1;
                    LexState::Comment(depth + 1)
                }
                (b'*', Some(b'/')) => {
                    index += 1;
                    if depth == 1 {
                        LexState::Code
                    } else {
                        LexState::Comment(depth - 1)
                    }
                }
                _ => LexState::Comment(depth),
            },
        };
        index += 1;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::split_batches;
    use crate::sql::ScriptVariables;

    fn texts(script: &str) -> Vec<String> {
        split_batches(script, ScriptVariables::Refuse)
            .unwrap()
            .into_iter()
            .map(|batch| batch.text)
            .collect()
    }

    #[test]
    fn a_script_without_go_is_one_batch() {
        let script = "SET NOCOUNT ON;\nSELECT 1;\n";
        assert_eq!(texts(script), vec![script.to_owned()]);
    }

    #[test]
    fn go_lines_separate_batches_in_any_case_and_indent() {
        let batches = split_batches(
            "SELECT 1;\nGO\n  select 2;\n\tgo  \nSELECT 3",
            ScriptVariables::Refuse,
        )
        .unwrap();
        let texts = batches
            .iter()
            .map(|batch| batch.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["SELECT 1;\n", "  select 2;\n", "SELECT 3"]);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.first_line)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
    }

    #[test]
    fn go_with_a_count_repeats_the_batch() {
        assert_eq!(
            texts("INSERT t DEFAULT VALUES;\nGO 3\n"),
            vec!["INSERT t DEFAULT VALUES;\n"; 3]
        );
        assert!(split_batches("SELECT 1\nGO 0\n", ScriptVariables::Refuse).is_err());
        assert!(split_batches("SELECT 1\nGO x\n", ScriptVariables::Refuse).is_err());
    }

    #[test]
    fn empty_batches_are_dropped() {
        assert_eq!(texts("GO\n\nGO\nSELECT 1\nGO\n   \n"), vec!["SELECT 1\n"]);
        assert!(texts("").is_empty());
        assert!(texts("\u{feff}\n").is_empty());
    }

    #[test]
    fn a_leading_bom_is_not_sent() {
        assert_eq!(texts("\u{feff}SELECT 1"), vec!["SELECT 1"]);
    }

    #[test]
    fn go_inside_literals_and_comments_is_text() {
        let script = "SELECT N'first\nGO\nlast';\n/* a\nGO\n /* nested */\nGO\n*/\nSELECT [odd\nGO\nname];\nGO\n";
        let batches = texts(script);
        assert_eq!(batches.len(), 1, "{batches:?}");
        assert!(batches[0].contains("N'first\nGO\nlast'"));
    }

    #[test]
    fn a_line_comment_does_not_open_a_literal() {
        assert_eq!(
            texts("SELECT 1 -- it's fine\nGO\nSELECT 2\n"),
            vec!["SELECT 1 -- it's fine\n", "SELECT 2\n"]
        );
    }

    #[test]
    fn go_followed_by_text_is_not_a_separator() {
        assert_eq!(texts("SELECT 1\nGO TO x\n"), vec!["SELECT 1\nGO TO x\n"]);
    }

    #[test]
    fn sqlcmd_commands_are_refused_with_their_line() {
        for (script, command) in [
            (":setvar DB x\nSELECT 1", ":setvar"),
            ("SELECT 1\nGO\n  :r other.sql\n", ":r"),
            (":ON ERROR EXIT\n", ":ON"),
            ("!!dir\n", "!!"),
        ] {
            let error = split_batches(script, ScriptVariables::Literal)
                .unwrap_err()
                .to_string();
            assert!(error.contains(command), "{error}");
            assert!(error.contains("--sqlcmd"), "{error}");
        }
        // A colon inside a literal or mid-line is T-SQL.
        assert_eq!(
            texts("SELECT N'\n:setvar is text here';\nSELECT CONVERT(time, '10:00')"),
            vec!["SELECT N'\n:setvar is text here';\nSELECT CONVERT(time, '10:00')"]
        );
    }

    #[test]
    fn scripting_variables_are_refused_unless_literal() {
        let script = "SELECT 1;\nUSE $(DB);\n";
        let error = split_batches(script, ScriptVariables::Refuse)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 2"), "{error}");
        assert_eq!(
            split_batches(script, ScriptVariables::Literal).unwrap()[0].text,
            script
        );
    }
}
