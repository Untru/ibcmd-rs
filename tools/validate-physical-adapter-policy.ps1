[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Join-Path $PSScriptRoot '..'),
    [switch]$SelfTest,
    [switch]$WriteBaseline
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$BaselineRelativePath = 'tools/physical-adapter-policy-baseline.json'
$AllowedCategories = @('uuid-literal', 'name-special-case', 'xml-policy')
$ExcludedMssqlModules = @('tests.rs', 'metadata_order_tests.rs', 'mxl_ir.rs', 'moxel.rs')

$scannerSource = @'
using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

public static class PhysicalAdapterPolicyScanner
{
    private sealed class Token
    {
        public string Kind;
        public string Text;
        public string Value;
        public Token(string kind, string text, string value = "") { Kind = kind; Text = text; Value = value; }
    }

    private struct Possibility
    {
        public bool CanTrue;
        public bool CanFalse;
        public Possibility(bool canTrue, bool canFalse) { CanTrue = canTrue; CanFalse = canFalse; }
    }

    private static readonly Regex Uuid = new Regex(
        @"(?i)(?<![0-9a-f])[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}(?![0-9a-f])",
        RegexOptions.Compiled);
    private static readonly Regex XmlContent = new Regex(
        @"(?is)<\s*[/!?A-Za-z_:]|\bxmlns(?::[A-Za-z_][A-Za-z0-9_.-]*)?\s*=",
        RegexOptions.Compiled);
    private static readonly Regex XmlFragment = new Regex(
        @"(?is)^\s*(?:<|>|/?>|[A-Za-z_:][A-Za-z0-9_.:-]*/?>)\s*$",
        RegexOptions.Compiled);

    public static string[] Scan(string source)
    {
        var counts = new Dictionary<string, int>(StringComparer.Ordinal);
        var tokens = RemoveTestOnlyItems(Tokenize(source));
        var current = new List<Token>();
        int parenDepth = 0, bracketDepth = 0;
        bool hasArrow = false;
        foreach (var token in tokens)
        {
            current.Add(token);
            if (token.Text == "(") parenDepth++;
            else if (token.Text == ")" && parenDepth > 0) parenDepth--;
            else if (token.Text == "[") bracketDepth++;
            else if (token.Text == "]" && bracketDepth > 0) bracketDepth--;
            else if (token.Text == "=>") hasArrow = true;

            bool boundary = token.Text == ";" || token.Text == "{" || token.Text == "}";
            if (token.Text == "," && hasArrow && parenDepth == 0 && bracketDepth == 0) boundary = true;
            if (boundary)
            {
                AnalyzeUnit(current, counts);
                current.Clear();
                hasArrow = false;
            }
        }
        AnalyzeUnit(current, counts);
        return counts.OrderBy(pair => pair.Key, StringComparer.Ordinal)
            .Select(pair => pair.Key + "|" + pair.Value.ToString(System.Globalization.CultureInfo.InvariantCulture))
            .ToArray();
    }

    private static void AnalyzeUnit(List<Token> unit, Dictionary<string, int> counts)
    {
        if (unit.Count == 0) return;
        string context = string.Join(" ", unit.Select(token => token.Text));
        var literals = unit.Where(token => token.Kind == "string" || token.Kind == "char").ToList();
        var strings = literals.Where(token => token.Kind == "string").ToList();

        foreach (var literal in strings)
            foreach (Match match in Uuid.Matches(literal.Value))
                Add(counts, "uuid-literal", context, match.Value.ToLowerInvariant());

        // A policy violation is a string used as an input to a name-routing
        // decision.  Do not classify every literal in the surrounding Rust unit:
        // the same unit may also contain diagnostic labels and formatted output
        // paths, neither of which selects a physical metadata name.
        for (int index = 0; index < unit.Count; index++)
        {
            var literal = unit[index];
            if (literal.Kind == "string" && !Uuid.IsMatch(literal.Value) &&
                IsNameRoutingLiteral(unit, index))
                Add(counts, "name-special-case", context, literal.Text);
        }

        var identifiers = unit.Where(token => token.Kind == "identifier")
            .Select(token => token.Text.ToLowerInvariant()).ToArray();
        bool xmlDecision = identifiers.Any(identifier =>
            identifier.Contains("qname") ||
            (identifier.Contains("xml") && (identifier.Contains("order") || identifier.Contains("default"))));
        bool schemaAccessor = identifiers.Any(identifier =>
            identifier == "from_raw_layout" ||
            Regex.IsMatch(identifier, @"^form_[a-z0-9_]+_schema$") ||
            identifier == "writerpolicy" ||
            Regex.IsMatch(identifier, @"^form_[a-z0-9_]+_xml_order$"));
        bool sink = HasMethod(unit, "push", "push_str") ||
            HasMacro(unit, identifier => identifier == "xml" || identifier == "write" ||
                identifier == "writeln" || identifier == "format" || identifier == "concat");
        string combined = string.Concat(literals.Select(literal => literal.Value));
        bool combinedXml = XmlContent.IsMatch(combined) || XmlFragment.IsMatch(combined);
        bool anyXmlFragment = literals.Any(literal => XmlContent.IsMatch(literal.Value) || XmlFragment.IsMatch(literal.Value));

        foreach (var literal in literals)
        {
            bool directXml = XmlContent.IsMatch(literal.Value);
            bool sinkFragment = sink && (combinedXml || anyXmlFragment || XmlFragment.IsMatch(literal.Value));
            if (directXml || sinkFragment || xmlDecision || schemaAccessor)
                Add(counts, "xml-policy", context, literal.Text);
        }
        if (xmlDecision || schemaAccessor)
            foreach (var literal in unit.Where(token =>
                token.Kind == "number" || (token.Kind == "identifier" && (token.Text == "true" || token.Text == "false"))))
                Add(counts, "xml-policy", context, literal.Text);
        if (HasMacro(unit, identifier => identifier == "xml") && literals.Count == 0)
            Add(counts, "xml-policy", context, context);
    }

    private static bool HasMethod(List<Token> tokens, params string[] names)
    {
        var accepted = new HashSet<string>(names, StringComparer.Ordinal);
        for (int index = 0; index + 2 < tokens.Count; index++)
            if (tokens[index].Text == "." && accepted.Contains(tokens[index + 1].Text) && tokens[index + 2].Text == "(")
                return true;
        return false;
    }

    private static bool IsNameRoutingLiteral(List<Token> tokens, int index)
    {
        string previous = index > 0 ? tokens[index - 1].Text : "";
        string next = index + 1 < tokens.Count ? tokens[index + 1].Text : "";
        if (previous == "==" || previous == "!=" || next == "==" || next == "!=")
            return true;
        if (previous == "|" || next == "|" || previous == ".." || next == ".." ||
            previous == "..=" || next == "..=")
            return true;
        if (next == "=>" || (previous == "let" && next == "="))
            return true;
        if (previous == "(" && index >= 3 && tokens[index - 3].Text == "." &&
            new[] { "eq", "ne", "contains", "starts_with", "ends_with" }
                .Contains(tokens[index - 2].Text))
            return true;
        for (int tokenIndex = 0; tokenIndex + 1 < index; tokenIndex++)
            if (tokens[tokenIndex].Kind == "identifier" &&
                tokens[tokenIndex].Text.IndexOf("match", StringComparison.OrdinalIgnoreCase) >= 0 &&
                tokens[tokenIndex + 1].Text == "!")
                return true;
        return false;
    }

    private static bool HasMacro(List<Token> tokens, Func<string, bool> predicate)
    {
        for (int index = 0; index + 1 < tokens.Count; index++)
            if (tokens[index].Kind == "identifier" && predicate(tokens[index].Text.ToLowerInvariant()) && tokens[index + 1].Text == "!")
                return true;
        return false;
    }

    private static void Add(Dictionary<string, int> counts, string category, string context, string literal)
    {
        context = NormalizeNewlines(context);
        literal = NormalizeNewlines(literal);
        string fingerprint;
        using (var sha = SHA256.Create())
            fingerprint = BitConverter.ToString(sha.ComputeHash(Encoding.UTF8.GetBytes(category + "\n" + context + "\n" + literal)))
                .Replace("-", "").ToLowerInvariant();
        string key = category + "|" + fingerprint;
        int count;
        counts.TryGetValue(key, out count);
        counts[key] = count + 1;
    }

    private static string NormalizeNewlines(string value)
    {
        return (value ?? "").Replace("\r\n", "\n").Replace("\r", "\n");
    }

    private static List<Token> RemoveTestOnlyItems(List<Token> tokens)
    {
        var result = new List<Token>(tokens.Count);
        int index = 0;
        while (index < tokens.Count)
        {
            int attributeEnd;
            Possibility predicate;
            if (TryParseCfgAttribute(tokens, index, out attributeEnd, out predicate) && !predicate.CanTrue)
            {
                int cursor = attributeEnd;
                while (cursor < tokens.Count && tokens[cursor].Text == "#")
                {
                    int ignoredEnd;
                    if (!TrySkipAttribute(tokens, cursor, out ignoredEnd)) break;
                    cursor = ignoredEnd;
                }
                int terminator = cursor;
                while (terminator < tokens.Count && tokens[terminator].Text != "{" && tokens[terminator].Text != ";")
                    terminator++;
                if (terminator < tokens.Count && tokens[terminator].Text == "{")
                {
                    int depth = 1;
                    terminator++;
                    while (terminator < tokens.Count && depth > 0)
                    {
                        if (tokens[terminator].Text == "{") depth++;
                        else if (tokens[terminator].Text == "}") depth--;
                        terminator++;
                    }
                    if (terminator < tokens.Count && tokens[terminator].Text == ";") terminator++;
                }
                else if (terminator < tokens.Count) terminator++;
                index = terminator;
                continue;
            }
            result.Add(tokens[index++]);
        }
        return result;
    }

    private static bool TrySkipAttribute(List<Token> tokens, int start, out int end)
    {
        end = start;
        if (start + 1 >= tokens.Count || tokens[start].Text != "#" || tokens[start + 1].Text != "[") return false;
        int depth = 1;
        int cursor = start + 2;
        while (cursor < tokens.Count && depth > 0)
        {
            if (tokens[cursor].Text == "[") depth++;
            else if (tokens[cursor].Text == "]") depth--;
            cursor++;
        }
        if (depth != 0) return false;
        end = cursor;
        return true;
    }

    private static bool TryParseCfgAttribute(List<Token> tokens, int start, out int end, out Possibility predicate)
    {
        predicate = new Possibility(true, true);
        if (!TrySkipAttribute(tokens, start, out end)) return false;
        if (start + 4 >= end || tokens[start + 2].Text != "cfg" || tokens[start + 3].Text != "(") return false;
        int expressionIndex = start + 4;
        int expressionEnd = end - 2;
        predicate = ParseCfgExpression(tokens, ref expressionIndex, expressionEnd);
        return expressionIndex == expressionEnd;
    }

    private static Possibility ParseCfgExpression(List<Token> tokens, ref int index, int end)
    {
        if (index >= end) return new Possibility(true, true);
        string name = tokens[index].Text;
        if ((name == "all" || name == "any" || name == "not") && index + 1 < end && tokens[index + 1].Text == "(")
        {
            index += 2;
            var children = new List<Possibility>();
            while (index < end && tokens[index].Text != ")")
            {
                children.Add(ParseCfgExpression(tokens, ref index, end));
                if (index < end && tokens[index].Text == ",") index++;
                else if (index < end && tokens[index].Text != ")") return new Possibility(true, true);
            }
            if (index >= end || tokens[index].Text != ")") return new Possibility(true, true);
            index++;
            if (name == "not")
            {
                if (children.Count != 1) return new Possibility(true, true);
                return new Possibility(children[0].CanFalse, children[0].CanTrue);
            }
            if (children.Count == 0) return name == "all" ? new Possibility(true, false) : new Possibility(false, true);
            if (name == "all")
                return new Possibility(children.All(child => child.CanTrue), children.Any(child => child.CanFalse));
            return new Possibility(children.Any(child => child.CanTrue), children.All(child => child.CanFalse));
        }
        if (name == "test")
        {
            index++;
            return new Possibility(false, true);
        }
        while (index < end && tokens[index].Text != "," && tokens[index].Text != ")") index++;
        return new Possibility(true, true);
    }

    private static List<Token> Tokenize(string text)
    {
        var tokens = new List<Token>(Math.Max(16, text.Length / 4));
        int index = 0;
        while (index < text.Length)
        {
            char ch = text[index];
            if (char.IsWhiteSpace(ch)) { index++; continue; }
            if (ch == '/' && index + 1 < text.Length && text[index + 1] == '/')
            {
                index += 2;
                while (index < text.Length && text[index] != '\n') index++;
                continue;
            }
            if (ch == '/' && index + 1 < text.Length && text[index + 1] == '*')
            {
                index += 2;
                int depth = 1;
                while (index < text.Length && depth > 0)
                {
                    if (index + 1 < text.Length && text[index] == '/' && text[index + 1] == '*') { depth++; index += 2; }
                    else if (index + 1 < text.Length && text[index] == '*' && text[index + 1] == '/') { depth--; index += 2; }
                    else index++;
                }
                continue;
            }

            int rawPrefix = ch == 'r' ? 1 : (ch == 'b' && index + 1 < text.Length && text[index + 1] == 'r' ? 2 : 0);
            if (rawPrefix > 0)
            {
                int quote = index + rawPrefix;
                while (quote < text.Length && text[quote] == '#') quote++;
                if (quote < text.Length && text[quote] == '"')
                {
                    int hashes = quote - index - rawPrefix;
                    string closing = "\"" + new string('#', hashes);
                    int contentStart = quote + 1;
                    int closingIndex = text.IndexOf(closing, contentStart, StringComparison.Ordinal);
                    if (closingIndex < 0) throw new InvalidOperationException("Rust tokenizer found an unterminated raw string.");
                    int tokenEnd = closingIndex + closing.Length;
                    tokens.Add(new Token("string", text.Substring(index, tokenEnd - index), text.Substring(contentStart, closingIndex - contentStart)));
                    index = tokenEnd;
                    continue;
                }
            }

            int normalPrefix = ch == '"' ? 0 : ((ch == 'b' || ch == 'c') && index + 1 < text.Length && text[index + 1] == '"' ? 1 : -1);
            if (normalPrefix >= 0)
            {
                int quote = index + normalPrefix, cursor = quote + 1;
                bool escaped = false;
                while (cursor < text.Length)
                {
                    if (!escaped && text[cursor] == '"') break;
                    if (!escaped && text[cursor] == '\\') escaped = true; else escaped = false;
                    cursor++;
                }
                if (cursor >= text.Length) throw new InvalidOperationException("Rust tokenizer found an unterminated string.");
                int tokenEnd = cursor + 1;
                tokens.Add(new Token("string", text.Substring(index, tokenEnd - index), Decode(text.Substring(quote + 1, cursor - quote - 1))));
                index = tokenEnd;
                continue;
            }

            if (ch == '\'')
            {
                int cursor = index + 1;
                bool escaped = false;
                while (cursor < text.Length && cursor - index <= 12)
                {
                    if (!escaped && text[cursor] == '\'') break;
                    if (!escaped && text[cursor] == '\\') escaped = true; else escaped = false;
                    cursor++;
                }
                if (cursor < text.Length && text[cursor] == '\'')
                {
                    string literal = text.Substring(index, cursor - index + 1);
                    tokens.Add(new Token("char", literal, Decode(text.Substring(index + 1, cursor - index - 1))));
                    index = cursor + 1;
                    continue;
                }
            }

            if (char.IsLetter(ch) || ch == '_')
            {
                int cursor = index + 1;
                while (cursor < text.Length && (char.IsLetterOrDigit(text[cursor]) || text[cursor] == '_')) cursor++;
                string identifier = text.Substring(index, cursor - index);
                tokens.Add(new Token("identifier", identifier, identifier));
                index = cursor;
                continue;
            }
            if (char.IsDigit(ch))
            {
                int cursor = index + 1;
                while (cursor < text.Length && (char.IsLetterOrDigit(text[cursor]) || text[cursor] == '_' || text[cursor] == '.')) cursor++;
                string number = text.Substring(index, cursor - index);
                tokens.Add(new Token("number", number, number));
                index = cursor;
                continue;
            }

            string op = null;
            if (index + 2 < text.Length && text.Substring(index, 3) == "..=") op = "..=";
            else if (index + 1 < text.Length)
            {
                string candidate = text.Substring(index, 2);
                if (new[] { "==", "!=", "=>", "::", "&&", "||", "<=", ">=", "->", ".." }.Contains(candidate)) op = candidate;
            }
            if (op != null) { tokens.Add(new Token("symbol", op)); index += op.Length; }
            else { tokens.Add(new Token("symbol", ch.ToString())); index++; }
        }
        return tokens;
    }

    private static string Decode(string value)
    {
        value = Regex.Replace(value, @"\\x([0-9a-fA-F]{2})", match => ((char)Convert.ToInt32(match.Groups[1].Value, 16)).ToString());
        value = Regex.Replace(value, @"\\u\{([0-9a-fA-F]{1,6})\}", match => char.ConvertFromUtf32(Convert.ToInt32(match.Groups[1].Value, 16)));
        return value.Replace("\\\"", "\"").Replace("\\n", "\n").Replace("\\r", "\r").Replace("\\t", "\t").Replace("\\\\", "\\");
    }
}
'@

if ($null -eq ('PhysicalAdapterPolicyScanner' -as [type])) {
    Add-Type -TypeDefinition $scannerSource -Language CSharp
}

function Get-Sha256 {
    param([Parameter(Mandatory)] [string]$Value)

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $algorithm.ComputeHash($bytes)
        return ([BitConverter]::ToString($hash)).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function ConvertFrom-RustEscapedString {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string]$Value)

    $decoded = [regex]::Replace($Value, '\\x([0-9a-fA-F]{2})', {
        param($match)
        return [string][char][Convert]::ToInt32($match.Groups[1].Value, 16)
    })
    $decoded = [regex]::Replace($decoded, '\\u\{([0-9a-fA-F]{1,6})\}', {
        param($match)
        return [char]::ConvertFromUtf32([Convert]::ToInt32($match.Groups[1].Value, 16))
    })
    return $decoded.Replace('\"', '"').Replace('\n', "`n").Replace('\r', "`r").Replace('\t', "`t").Replace('\\', '\')
}

function New-RustToken {
    param(
        [Parameter(Mandatory)] [string]$Kind,
        [Parameter(Mandatory)] [AllowEmptyString()] [string]$Text,
        [AllowEmptyString()] [string]$Value = ''
    )
    return [pscustomobject]@{ Kind = $Kind; Text = $Text; Value = $Value }
}

function Get-RustTokens {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string]$Text)

    $tokens = [System.Collections.Generic.List[object]]::new()
    $index = 0
    while ($index -lt $Text.Length) {
        $character = $Text[$index]
        if ([char]::IsWhiteSpace($character)) {
            $index++
            continue
        }

        if ($character -eq '/' -and $index + 1 -lt $Text.Length -and $Text[$index + 1] -eq '/') {
            $index += 2
            while ($index -lt $Text.Length -and $Text[$index] -ne "`n") { $index++ }
            continue
        }
        if ($character -eq '/' -and $index + 1 -lt $Text.Length -and $Text[$index + 1] -eq '*') {
            $index += 2
            $commentDepth = 1
            while ($index -lt $Text.Length -and $commentDepth -gt 0) {
                if ($index + 1 -lt $Text.Length -and $Text[$index] -eq '/' -and $Text[$index + 1] -eq '*') {
                    $commentDepth++
                    $index += 2
                }
                elseif ($index + 1 -lt $Text.Length -and $Text[$index] -eq '*' -and $Text[$index + 1] -eq '/') {
                    $commentDepth--
                    $index += 2
                }
                else {
                    $index++
                }
            }
            continue
        }

        $rawPrefixLength = 0
        if ($character -eq 'r') {
            $rawPrefixLength = 1
        }
        elseif ($character -eq 'b' -and $index + 1 -lt $Text.Length -and $Text[$index + 1] -eq 'r') {
            $rawPrefixLength = 2
        }
        if ($rawPrefixLength -gt 0) {
            $quoteIndex = $index + $rawPrefixLength
            while ($quoteIndex -lt $Text.Length -and $Text[$quoteIndex] -eq '#') { $quoteIndex++ }
            if ($quoteIndex -lt $Text.Length -and $Text[$quoteIndex] -eq '"') {
                $hashCount = $quoteIndex - ($index + $rawPrefixLength)
                $closing = '"' + ('#' * $hashCount)
                $contentStart = $quoteIndex + 1
                $closingIndex = $Text.IndexOf($closing, $contentStart, [System.StringComparison]::Ordinal)
                if ($closingIndex -lt 0) {
                    throw 'Rust tokenizer found an unterminated raw string.'
                }
                $tokenEnd = $closingIndex + $closing.Length
                $tokens.Add((New-RustToken -Kind 'string' -Text $Text.Substring($index, $tokenEnd - $index) -Value $Text.Substring($contentStart, $closingIndex - $contentStart)))
                $index = $tokenEnd
                continue
            }
        }

        $normalPrefixLength = 0
        if ($character -eq '"') {
            $normalPrefixLength = 0
        }
        elseif (($character -eq 'b' -or $character -eq 'c') -and $index + 1 -lt $Text.Length -and $Text[$index + 1] -eq '"') {
            $normalPrefixLength = 1
        }
        else {
            $normalPrefixLength = -1
        }
        if ($normalPrefixLength -ge 0) {
            $quoteIndex = $index + $normalPrefixLength
            $cursor = $quoteIndex + 1
            $escaped = $false
            while ($cursor -lt $Text.Length) {
                if (-not $escaped -and $Text[$cursor] -eq '"') { break }
                if (-not $escaped -and $Text[$cursor] -eq '\') {
                    $escaped = $true
                }
                else {
                    $escaped = $false
                }
                $cursor++
            }
            if ($cursor -ge $Text.Length) {
                throw 'Rust tokenizer found an unterminated string.'
            }
            $tokenEnd = $cursor + 1
            $value = ConvertFrom-RustEscapedString $Text.Substring($quoteIndex + 1, $cursor - $quoteIndex - 1)
            $tokens.Add((New-RustToken -Kind 'string' -Text $Text.Substring($index, $tokenEnd - $index) -Value $value))
            $index = $tokenEnd
            continue
        }

        if ([char]::IsLetter($character) -or $character -eq '_') {
            $cursor = $index + 1
            while ($cursor -lt $Text.Length -and ([char]::IsLetterOrDigit($Text[$cursor]) -or $Text[$cursor] -eq '_')) { $cursor++ }
            $identifier = $Text.Substring($index, $cursor - $index)
            $tokens.Add((New-RustToken -Kind 'identifier' -Text $identifier -Value $identifier))
            $index = $cursor
            continue
        }
        if ([char]::IsDigit($character)) {
            $cursor = $index + 1
            while ($cursor -lt $Text.Length -and ([char]::IsLetterOrDigit($Text[$cursor]) -or $Text[$cursor] -in @('_', '.'))) { $cursor++ }
            $number = $Text.Substring($index, $cursor - $index)
            $tokens.Add((New-RustToken -Kind 'number' -Text $number -Value $number))
            $index = $cursor
            continue
        }

        $operator = $null
        if ($index + 1 -lt $Text.Length) {
            $candidate = $Text.Substring($index, 2)
            if ($candidate -in @('==', '!=', '=>', '::', '&&', '||', '<=', '>=', '->')) {
                $operator = $candidate
            }
        }
        if ($null -ne $operator) {
            $tokens.Add((New-RustToken -Kind 'symbol' -Text $operator))
            $index += 2
        }
        else {
            $tokens.Add((New-RustToken -Kind 'symbol' -Text ([string]$character)))
            $index++
        }
    }
    return @($tokens)
}

function Remove-TestRustTokens {
    param([Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Tokens)

    $result = [System.Collections.Generic.List[object]]::new()
    $index = 0
    while ($index -lt $Tokens.Count) {
        if ($index + 1 -lt $Tokens.Count -and $Tokens[$index].Text -eq '#' -and $Tokens[$index + 1].Text -eq '[') {
            $depth = 1
            $attributeEnd = $index + 2
            while ($attributeEnd -lt $Tokens.Count -and $depth -gt 0) {
                if ($Tokens[$attributeEnd].Text -eq '[') { $depth++ }
                elseif ($Tokens[$attributeEnd].Text -eq ']') { $depth-- }
                $attributeEnd++
            }
            $attributeText = (($Tokens[$index..($attributeEnd - 1)] | ForEach-Object Text) -join '')
            if ($depth -eq 0 -and $attributeText -match '^#\[cfg\(.*\btest\b.*\)\]$') {
                $cursor = $attributeEnd
                while ($cursor -lt $Tokens.Count -and $Tokens[$cursor].Text -eq '#') {
                    $cursor++
                    if ($cursor -lt $Tokens.Count -and $Tokens[$cursor].Text -eq '[') {
                        $nested = 1
                        $cursor++
                        while ($cursor -lt $Tokens.Count -and $nested -gt 0) {
                            if ($Tokens[$cursor].Text -eq '[') { $nested++ }
                            elseif ($Tokens[$cursor].Text -eq ']') { $nested-- }
                            $cursor++
                        }
                    }
                }
                while ($cursor -lt $Tokens.Count -and $Tokens[$cursor].Text -ne '{' -and $Tokens[$cursor].Text -ne ';') { $cursor++ }
                if ($cursor -lt $Tokens.Count -and $Tokens[$cursor].Text -eq '{') {
                    $bodyDepth = 1
                    $cursor++
                    while ($cursor -lt $Tokens.Count -and $bodyDepth -gt 0) {
                        if ($Tokens[$cursor].Text -eq '{') { $bodyDepth++ }
                        elseif ($Tokens[$cursor].Text -eq '}') { $bodyDepth-- }
                        $cursor++
                    }
                }
                elseif ($cursor -lt $Tokens.Count) {
                    $cursor++
                }
                $index = $cursor
                continue
            }
        }
        $result.Add($Tokens[$index])
        $index++
    }
    return @($result)
}

function Get-RustLogicalUnits {
    param([Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Tokens)

    $units = [System.Collections.Generic.List[object]]::new()
    $current = [System.Collections.Generic.List[object]]::new()
    $parenDepth = 0
    $bracketDepth = 0
    $hasMatchArrow = $false
    foreach ($token in $Tokens) {
        $current.Add($token)
        switch ($token.Text) {
            '(' { $parenDepth++ }
            ')' { if ($parenDepth -gt 0) { $parenDepth-- } }
            '[' { $bracketDepth++ }
            ']' { if ($bracketDepth -gt 0) { $bracketDepth-- } }
            '=>' { $hasMatchArrow = $true }
        }
        $boundary = $token.Text -in @(';', '{', '}')
        if ($token.Text -eq ',' -and $hasMatchArrow -and $parenDepth -eq 0 -and $bracketDepth -eq 0) {
            $boundary = $true
        }
        if ($boundary) {
            $units.Add([pscustomobject]@{ Tokens = @($current) })
            $current.Clear()
            $hasMatchArrow = $false
        }
    }
    if ($current.Count -gt 0) {
        $units.Add([pscustomobject]@{ Tokens = @($current) })
    }
    return @($units)
}

function Get-ScopedFiles {
    param([Parameter(Mandatory)] [string]$Root)

    $files = [System.Collections.Generic.List[System.IO.FileInfo]]::new()
    $moduleBlob = Join-Path $Root 'src/module_blob.rs'
    if (-not (Test-Path -LiteralPath $moduleBlob -PathType Leaf)) {
        throw 'Scoped source file is missing.'
    }
    $files.Add((Get-Item -LiteralPath $moduleBlob))

    $mssqlRoot = Join-Path $Root 'src/mssql_dump'
    if (-not (Test-Path -LiteralPath $mssqlRoot -PathType Container)) {
        throw 'Scoped source directory is missing.'
    }
    foreach ($file in Get-ChildItem -LiteralPath $mssqlRoot -Filter '*.rs' -File -Recurse) {
        if ($ExcludedMssqlModules -notcontains $file.Name) {
            $files.Add($file)
        }
    }
    return @($files | Sort-Object FullName)
}

function Get-LogicalPath {
    param(
        [Parameter(Mandatory)] [string]$Root,
        [Parameter(Mandatory)] [string]$Path
    )

    $rootPath = [System.IO.Path]::GetFullPath($Root)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $relativePathMethod = [System.IO.Path].GetMethod(
        'GetRelativePath', [type[]]@([string], [string]))
    if ($null -ne $relativePathMethod) {
        return [System.IO.Path]::GetRelativePath($rootPath, $fullPath).Replace('\', '/')
    }

    $separator = [System.IO.Path]::DirectorySeparatorChar
    $rootPrefix = $rootPath.TrimEnd([char[]]@('/', '\')) + $separator
    $comparison = if ($separator -eq '\') {
        [System.StringComparison]::OrdinalIgnoreCase
    } else {
        [System.StringComparison]::Ordinal
    }
    if (-not $fullPath.StartsWith($rootPrefix, $comparison)) {
        throw 'Scoped source path is outside repository root.'
    }
    return $fullPath.Substring($rootPrefix.Length).Replace('\', '/')
}

function Add-Occurrence {
    param(
        [Parameter(Mandatory)] [hashtable]$Counts,
        [Parameter(Mandatory)] [string]$Category,
        [Parameter(Mandatory)] [string]$Context,
        [Parameter(Mandatory)] [string]$Literal
    )

    $fingerprint = Get-Sha256 "$Category`n$Context`n$Literal"
    $key = "$Category|$fingerprint"
    if ($Counts.ContainsKey($key)) {
        $Counts[$key].count++
    }
    else {
        $Counts[$key] = [ordered]@{ category = $Category; fingerprint = $fingerprint; count = 1 }
    }
}

function Get-FileInventory {
    param([Parameter(Mandatory)] [string]$Path)

    $text = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
    if ($null -eq $text) { $text = '' }
    foreach ($record in [PhysicalAdapterPolicyScanner]::Scan($text)) {
        $lastSeparator = $record.LastIndexOf('|')
        $firstSeparator = $record.IndexOf('|')
        if ($firstSeparator -lt 1 -or $lastSeparator -le $firstSeparator) {
            throw 'Physical-adapter scanner returned an invalid occurrence record.'
        }
        [ordered]@{
            category = $record.Substring(0, $firstSeparator)
            fingerprint = $record.Substring($firstSeparator + 1, $lastSeparator - $firstSeparator - 1)
            count = [long]$record.Substring($lastSeparator + 1)
        }
    }
}

function New-InventoryDocument {
    param([Parameter(Mandatory)] [string]$Root)

    $files = [System.Collections.Generic.List[object]]::new()
    foreach ($file in Get-ScopedFiles $Root) {
        $files.Add([ordered]@{
            file = Get-LogicalPath -Root $Root -Path $file.FullName
            occurrences = @(Get-FileInventory -Path $file.FullName)
        })
    }
    return [ordered]@{ schemaVersion = 1; files = @($files) }
}

function Read-Baseline {
    param([Parameter(Mandatory)] [string]$Root)

    $path = Join-Path $Root $BaselineRelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw 'Physical-adapter baseline is missing.'
    }
    try {
        $json = Get-Content -LiteralPath $path -Raw -Encoding UTF8
        $convertFromJson = Get-Command ConvertFrom-Json
        if ($convertFromJson.Parameters.ContainsKey('AsHashtable')) {
            $document = ($json | ConvertFrom-Json -AsHashtable -Depth 100)
        }
        else {
            Add-Type -AssemblyName System.Web.Extensions
            $serializer = New-Object System.Web.Script.Serialization.JavaScriptSerializer
            $serializer.MaxJsonLength = [int]::MaxValue
            $document = $serializer.DeserializeObject($json)
            foreach ($file in $document['files']) {
                foreach ($occurrence in $file['occurrences']) {
                    $occurrence['count'] = [long]$occurrence['count']
                }
            }
        }
    }
    catch {
        throw 'Physical-adapter baseline is not valid JSON.'
    }
    if (-not ($document -is [System.Collections.IDictionary]) -or $document['schemaVersion'] -ne 1 -or
        -not ($document['files'] -is [System.Collections.IEnumerable])) {
        throw 'Physical-adapter baseline has an unsupported shape.'
    }
    return $document
}

function Assert-BaselineShape {
    param([Parameter(Mandatory)] [System.Collections.IDictionary]$Baseline)

    $seenFiles = @{}
    foreach ($file in $Baseline['files']) {
        if (-not ($file -is [System.Collections.IDictionary]) -or -not ($file['file'] -is [string]) -or
            [string]::IsNullOrWhiteSpace($file['file']) -or $seenFiles.ContainsKey($file['file']) -or
            -not ($file['occurrences'] -is [System.Collections.IEnumerable])) {
            throw 'Physical-adapter baseline has an invalid file record.'
        }
        $seenFiles[$file['file']] = $true
        $seenOccurrences = @{}
        foreach ($occurrence in $file['occurrences']) {
            if (-not ($occurrence -is [System.Collections.IDictionary]) -or
                $AllowedCategories -notcontains $occurrence['category'] -or
                -not ([string]$occurrence['fingerprint'] -match '^[0-9a-f]{64}$') -or
                -not ($occurrence['count'] -is [long]) -or [long]$occurrence['count'] -lt 1) {
                throw 'Physical-adapter baseline has an invalid occurrence record.'
            }
            $key = "$($occurrence['category'])|$($occurrence['fingerprint'])"
            if ($seenOccurrences.ContainsKey($key)) {
                throw 'Physical-adapter baseline duplicates an occurrence record.'
            }
            $seenOccurrences[$key] = $true
        }
    }
}

function Assert-InventoryAllowed {
    param(
        [Parameter(Mandatory)] [System.Collections.IDictionary]$Baseline,
        [Parameter(Mandatory)] [System.Collections.IDictionary]$Current
    )

    Assert-BaselineShape $Baseline
    $baselineByFile = @{}
    foreach ($file in $Baseline['files']) { $baselineByFile[$file['file']] = $file }
    foreach ($file in $Current['files']) {
        $fileHash = Get-Sha256 $file['file']
        if (-not $baselineByFile.ContainsKey($file['file'])) {
            throw "Physical-adapter policy guard rejected unbaselined file_sha256=$fileHash."
        }
        $allowed = @{}
        foreach ($occurrence in $baselineByFile[$file['file']]['occurrences']) {
            $allowed["$($occurrence['category'])|$($occurrence['fingerprint'])"] = [long]$occurrence['count']
        }
        foreach ($occurrence in $file['occurrences']) {
            $key = "$($occurrence['category'])|$($occurrence['fingerprint'])"
            if (-not $allowed.ContainsKey($key) -or [long]$occurrence['count'] -gt $allowed[$key]) {
                throw "Physical-adapter policy guard rejected category=$($occurrence['category']) file_sha256=$fileHash fingerprint_sha256=$($occurrence['fingerprint'])."
            }
        }
    }
    foreach ($file in $Baseline['files']) {
        if (-not (@($Current['files'] | Where-Object { $_['file'] -eq $file['file'] }).Count)) {
            throw 'Physical-adapter policy guard baseline references a missing scoped file.'
        }
    }
}

function Write-BaselineDocument {
    param(
        [Parameter(Mandatory)] [string]$Root,
        [Parameter(Mandatory)] [System.Collections.IDictionary]$Document
    )

    $path = Join-Path $Root $BaselineRelativePath
    $json = $Document | ConvertTo-Json -Depth 100 -Compress
    [System.IO.File]::WriteAllText($path, $json + "`n", [System.Text.UTF8Encoding]::new($false))
}

function Assert-Rejected {
    param(
        [Parameter(Mandatory)] [scriptblock]$Action,
        [Parameter(Mandatory)] [string]$Expected
    )

    try { & $Action }
    catch {
        if ($_.Exception.Message -notmatch [regex]::Escape($Expected)) {
            throw "Synthetic self-test used an unexpected rejection branch: $($_.Exception.Message)"
        }
        return
    }
    throw "Synthetic self-test unexpectedly succeeded for expected=$Expected."
}

function Invoke-SelfTest {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ("ibcmd-physical-policy-" + [guid]::NewGuid())
    [System.IO.Directory]::CreateDirectory((Join-Path $root 'src/mssql_dump')) | Out-Null
    [System.IO.Directory]::CreateDirectory((Join-Path $root 'tools')) | Out-Null
    try {
        $moduleBlob = Join-Path $root 'src/module_blob.rs'
        $formBody = Join-Path $root 'src/mssql_dump/form_body.rs'
        $moduleBlobText = "const ID: &str = `"11111111-1111-4111-8111-111111111111`";`n"
        $formBodyText = @'
fn existing(kind: &str) { if kind == "Existing" { } }
#[cfg(test)]
mod tests { const TEST_ONLY: &str = "22222222-2222-4222-8222-222222222222"; }
#[cfg(all(test, feature = "synthetic"))]
const ALL_TEST_ONLY: &str = "23232323-2323-4232-8232-232323232323";
#[cfg(any(all(test, feature = "left"), all(test, feature = "right")))]
const EVERY_BRANCH_TEST_ONLY: &str = "24242424-2424-4242-8242-242424242424";
fn schema(fields: &[&str]) { FormRootSchema::from_raw_layout(fields); }
fn multiline(xml: &mut String) {
    xml.push_str(r#"<Root>
<Child/>
</Root>"#);
}
'@
        $formBodyText = $formBodyText.Replace("`r`n", "`n").Replace("`r", "`n")
        [System.IO.File]::WriteAllText($moduleBlob, $moduleBlobText, [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($formBody, $formBodyText, [System.Text.UTF8Encoding]::new($false))
        $baseline = New-InventoryDocument $root
        Write-BaselineDocument -Root $root -Document $baseline
        $baselinePath = Join-Path $root $BaselineRelativePath
        $lfBaseline = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($baselinePath))
        [System.IO.File]::WriteAllText($moduleBlob, $moduleBlobText.Replace("`n", "`r`n"), [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($formBody, $formBodyText.Replace("`n", "`r`n"), [System.Text.UTF8Encoding]::new($false))
        $crlfInventory = New-InventoryDocument $root
        Assert-InventoryAllowed -Baseline $baseline -Current $crlfInventory
        Write-BaselineDocument -Root $root -Document $crlfInventory
        $crlfBaseline = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($baselinePath))
        if ($lfBaseline -ne $crlfBaseline) {
            throw 'LF and CRLF sources produced different canonical baselines.'
        }
        [System.IO.File]::WriteAllText($moduleBlob, $moduleBlobText, [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($formBody, $formBodyText, [System.Text.UTF8Encoding]::new($false))
        Write-BaselineDocument -Root $root -Document $baseline
        Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root)
        if (@(Get-FileInventory $formBody | Where-Object category -eq 'uuid-literal').Count -ne 0) {
            throw 'Synthetic test-code or schema-accessor exclusion failed.'
        }
        $delimiterOnlySpecialCases = @(
            [PhysicalAdapterPolicyScanner]::Scan(
                'fn canonical_binding_key(kind: &str, uuid: &str) -> bool { kind != uuid && !uuid.contains(''|'') }'
            ) | Where-Object { $_ -like 'name-special-case|*' }
        )
        if ($delimiterOnlySpecialCases.Count -ne 0) {
            throw 'Character delimiters were incorrectly classified as name special cases.'
        }
        $diagnosticErrorSpecialCases = @(
            [PhysicalAdapterPolicyScanner]::Scan(
                'fn error_class(error: ParseError) -> &''static str { match error { ParseError::Broken => "broken_payload", } }'
            ) | Where-Object { $_ -like 'name-special-case|*' }
        )
        if ($diagnosticErrorSpecialCases.Count -ne 0) {
            throw 'Enum error labels were incorrectly classified as name special cases.'
        }

        Add-Content -LiteralPath $moduleBlob 'const EXTRA: &str = "33333333-3333-4333-8333-333333333333";'
        Assert-Rejected { Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root) } 'category=uuid-literal'
        [System.IO.File]::WriteAllText($moduleBlob, "const ID: &str = `"44444444-4444-4444-8444-444444444444`";`n", [System.Text.UTF8Encoding]::new($false))
        Assert-Rejected { Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root) } 'category=uuid-literal'
        [System.IO.File]::WriteAllText($moduleBlob, "const ID: &str = `"11111111-1111-4111-8111-111111111111`";`n", [System.Text.UTF8Encoding]::new($false))

        $adversarialCases = @(
            @{ Category = 'name-special-case'; Source = 'fn a(kind: &str) { if kind == "Catalog.New" { } }' },
            @{ Category = 'name-special-case'; Source = 'fn b(object_name: &str) { if "Catalog.New" == object_name { } }' },
            @{ Category = 'name-special-case'; Source = 'fn c(object_name: &str) { object_name.eq("Catalog.New"); }' },
            @{ Category = 'name-special-case'; Source = 'fn d(value: &str) { value.ne("Catalog.New"); value.contains("Needle"); value.starts_with("Prefix"); value.ends_with("Suffix"); }' },
            @{ Category = 'name-special-case'; Source = 'fn e(value: &str) { match value { "Catalog.New" if value.contains("Guard") => true, _ => false } }' },
            @{ Category = 'name-special-case'; Source = 'fn e2(value: &str) { matches!(value, "Catalog.New" | "Document.New"); }' },
            @{ Category = 'name-special-case'; Source = 'fn e3(value: &str) { if let "Catalog.New" = value { } while let "Document.New" = value { } }' },
            @{ Category = 'name-special-case'; Source = 'fn e4(value: &str) { let "Catalog.New" = value else { return; }; }' },
            @{ Category = 'name-special-case'; Source = 'fn e5(value: &str) { match value { "A" | "B" => true, "C"..="Z" => false, _ => false } }' },
            @{ Category = 'name-special-case'; Source = 'fn e6(value: &str) { token_match!(value, "Catalog.New"); }' },
            @{ Category = 'xml-policy'; Source = "fn f() { let neutral = xml_order(`n    `"NewChild`",`n    7,`n); }" },
            @{ Category = 'xml-policy'; Source = 'fn g(xml: &mut String) { xml.push_str("<NewChild>"); }' },
            @{ Category = 'xml-policy'; Source = 'fn h(fields: &[&str]) { FormRootSchema::from_raw_layout("local-sensitive-literal"); }' },
            @{ Category = 'xml-policy'; Source = 'fn i(xml: &mut String) { xml.push_str(r#"<RawChild/>"#); }' },
            @{ Category = 'xml-policy'; Source = 'fn j(xml: &mut String) { xml.push_str("\x3cEscapedChild/>"); }' },
            @{ Category = 'xml-policy'; Source = 'fn k(xml: &mut String) { write!(xml, /* macro comment */ "<MacroChild/>"); }' },
            @{ Category = 'xml-policy'; Source = "fn l(xml: &mut String) { xml.push('<'); xml.push_str(`"FragmentChild/>`"); }" },
            @{ Category = 'xml-policy'; Source = 'fn m(xml: &mut String) { xml.push_str(concat!("<", "ConcatChild/>")); }' },
            @{ Category = 'xml-policy'; Source = 'fn n(xml: &mut String) { write!(xml, "{}{}", "<", "WriteChild/>"); }' },
            @{ Category = 'xml-policy'; Source = 'fn o() { let value = format!("{}{}", "<", "FormatChild/>"); }' },
            @{ Category = 'uuid-literal'; Source = '#[cfg(not(test))] const LIVE_NOT_TEST: &str = "31313131-3131-4313-8313-313131313131";' },
            @{ Category = 'uuid-literal'; Source = '#[cfg(any(test, feature = "synthetic"))] const LIVE_ANY: &str = "32323232-3232-4323-8323-323232323232";' },
            @{ Category = 'uuid-literal'; Source = '#[cfg_attr(test, allow(dead_code))] const AMBIGUOUS_ATTR: &str = "33333333-3333-4333-8333-333333333334";' }
        )
        foreach ($case in $adversarialCases) {
            [System.IO.File]::WriteAllText($formBody, $formBodyText + "`n" + $case.Source, [System.Text.UTF8Encoding]::new($false))
            Assert-Rejected {
                Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root)
            } "category=$($case.Category)"
        }
        [System.IO.File]::WriteAllText($formBody, $formBodyText + "`n" + 'fn existing(kind: &str) { if kind == "Existing" { } }', [System.Text.UTF8Encoding]::new($false))
        Assert-Rejected {
            Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root)
        } 'category=name-special-case'

        [System.IO.File]::WriteAllText($moduleBlob, '', [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText($formBody, '', [System.Text.UTF8Encoding]::new($false))
        Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root)

        [System.IO.File]::WriteAllText((Join-Path $root 'src/mssql_dump/fetch.rs'), 'fn fetch() {}', [System.Text.UTF8Encoding]::new($false))
        Assert-Rejected { Assert-InventoryAllowed -Baseline (Read-Baseline $root) -Current (New-InventoryDocument $root) } 'unbaselined file_sha256='
        Remove-Item -LiteralPath (Join-Path $root 'src/mssql_dump/fetch.rs') -Force

        [System.IO.File]::WriteAllText($baselinePath, '{', [System.Text.UTF8Encoding]::new($false))
        Assert-Rejected { Read-Baseline $root } 'not valid JSON'
    }
    finally {
        if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    }
    Write-Host 'Physical-adapter policy guard self-tests passed.'
}

$resolvedRoot = [System.IO.Path]::GetFullPath($RepositoryRoot)
if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

$inventory = New-InventoryDocument $resolvedRoot
if ($WriteBaseline) {
    Write-BaselineDocument -Root $resolvedRoot -Document $inventory
    Write-Host 'Physical-adapter policy baseline written.'
    exit 0
}

Assert-InventoryAllowed -Baseline (Read-Baseline $resolvedRoot) -Current $inventory
Write-Host 'Physical-adapter policy guard passed.'
