# Shared helpers of the apply-trace capture kit (dot-source this file).
#
# Everything here talks to SQL Server through System.Data.SqlClient with
# Windows authentication (PowerShell 7 ships the assembly; Windows PowerShell
# has it in the GAC), so the kit needs neither sqlcmd nor bcp on PATH.
# Nothing in the kit writes to the database it inspects: snapshots are
# read-only, traces only create and drop an Extended Events session on the
# server.

Set-StrictMode -Version Latest

$script:KitVersion = '1'
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-Log {
    param([string]$Message)
    Write-Host ('[{0}] {1}' -f (Get-Date -Format 'HH:mm:ss'), $Message)
}

function Get-ConnectionString {
    param([string]$Server = 'localhost', [string]$Database = 'master', [string]$App = 'ibcmd-rs apply-trace kit')
    return "Server=$Server;Database=$Database;Integrated Security=True;TrustServerCertificate=True;Application Name=$App;Connect Timeout=30"
}

function Open-Sql {
    param([string]$Server = 'localhost', [string]$Database = 'master', [string]$App = 'ibcmd-rs apply-trace kit')
    $conn = New-Object System.Data.SqlClient.SqlConnection (Get-ConnectionString -Server $Server -Database $Database -App $App)
    $conn.Open()
    return $conn
}

# Runs a statement that returns no rows. Parameters: hashtable name -> value.
function Invoke-SqlNonQuery {
    param($Conn, [string]$Sql, [hashtable]$Params = @{}, [int]$Timeout = 600)
    $cmd = $Conn.CreateCommand()
    $cmd.CommandText = $Sql
    $cmd.CommandTimeout = $Timeout
    foreach ($k in $Params.Keys) { [void]$cmd.Parameters.AddWithValue($k, $Params[$k]) }
    [void]$cmd.ExecuteNonQuery()
}

function Invoke-SqlScalar {
    param($Conn, [string]$Sql, [hashtable]$Params = @{}, [int]$Timeout = 600)
    $cmd = $Conn.CreateCommand()
    $cmd.CommandText = $Sql
    $cmd.CommandTimeout = $Timeout
    foreach ($k in $Params.Keys) { [void]$cmd.Parameters.AddWithValue($k, $Params[$k]) }
    $v = $cmd.ExecuteScalar()
    if ($v -is [System.DBNull]) { return $null }
    return $v
}

# Returns every row as an ordered hashtable (use only for small result sets).
function Invoke-SqlRows {
    param($Conn, [string]$Sql, [hashtable]$Params = @{}, [int]$Timeout = 600)
    $cmd = $Conn.CreateCommand()
    $cmd.CommandText = $Sql
    $cmd.CommandTimeout = $Timeout
    foreach ($k in $Params.Keys) { [void]$cmd.Parameters.AddWithValue($k, $Params[$k]) }
    $reader = $cmd.ExecuteReader()
    $rows = New-Object System.Collections.Generic.List[object]
    try {
        while ($reader.Read()) {
            $row = [ordered]@{}
            for ($i = 0; $i -lt $reader.FieldCount; $i++) {
                $v = $reader.GetValue($i)
                if ($v -is [System.DBNull]) { $v = $null }
                $row[$reader.GetName($i)] = $v
            }
            $rows.Add($row)
        }
    } finally { $reader.Close() }
    return , $rows
}

# TSV escaping shared by every file the kit writes: backslash, tab, CR, LF.
function ConvertTo-TsvField {
    param($Value)
    if ($null -eq $Value) { return '' }
    $s = [string]$Value
    if ($s.IndexOfAny([char[]]@("`t", "`r", "`n", '\')) -lt 0) { return $s }
    return $s.Replace('\', '\\').Replace("`t", '\t').Replace("`r", '\r').Replace("`n", '\n')
}

function New-Utf8Writer {
    param([string]$Path)
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    return New-Object System.IO.StreamWriter($Path, $false, $script:Utf8NoBom)
}

function Write-TsvLine {
    param($Writer, [object[]]$Fields)
    $Writer.Write(((($Fields | ForEach-Object { ConvertTo-TsvField $_ }) -join "`t") + "`n"))
}

# The hot loops (hashing, hex, inflate, classification) are compiled C#: a
# snapshot handles tens of thousands of rows.  Conservative C# 5 syntax so the
# same file compiles under Windows PowerShell and PowerShell 7.
if (-not ('ApplyTraceKit.Codec' -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace ApplyTraceKit
{
    // Content-addressed store of row bytes: append-only pack files plus one index.
    //   <store>\index.tsv          sha256 <TAB> pack file <TAB> offset <TAB> gzip length <TAB> raw length
    //   <store>\pack-<stamp>.bin   concatenated gzip members
    // One pack file per writer process; the index is shared (lines are appended whole).
    public sealed class BlobPack : IDisposable
    {
        private readonly string dir;
        private readonly string indexPath;
        private readonly string packName;
        private readonly FileStream pack;
        private readonly System.Collections.Generic.HashSet<string> known = new System.Collections.Generic.HashSet<string>();
        private readonly StringBuilder pending = new StringBuilder();
        public long AddedBytes;
        public int AddedCount;

        public BlobPack(string storeDir)
        {
            dir = storeDir;
            Directory.CreateDirectory(dir);
            indexPath = Path.Combine(dir, "index.tsv");
            if (File.Exists(indexPath))
            {
                using (FileStream fs = new FileStream(indexPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
                using (StreamReader sr = new StreamReader(fs, new UTF8Encoding(false)))
                {
                    string line;
                    while ((line = sr.ReadLine()) != null)
                    {
                        int tab = line.IndexOf('\t');
                        if (tab == 64) known.Add(line.Substring(0, 64));
                    }
                }
            }
            packName = "pack-" + DateTime.UtcNow.ToString("yyyyMMddHHmmss") + "-" + System.Diagnostics.Process.GetCurrentProcess().Id + ".bin";
            pack = new FileStream(Path.Combine(dir, packName), FileMode.CreateNew, FileAccess.Write, FileShare.Read);
        }

        public bool Has(string sha) { return known.Contains(sha); }

        // true when the blob was new
        public bool Add(string sha, byte[] data)
        {
            if (known.Contains(sha)) return false;
            long off = pack.Position;
            using (System.IO.Compression.GZipStream gz = new System.IO.Compression.GZipStream(pack, System.IO.Compression.CompressionLevel.Fastest, true))
            {
                gz.Write(data, 0, data.Length);
            }
            long len = pack.Position - off;
            pending.Append(sha).Append('\t').Append(packName).Append('\t').Append(off).Append('\t').Append(len).Append('\t').Append(data.Length).Append('\n');
            known.Add(sha);
            AddedBytes += data.Length;
            AddedCount++;
            if (pending.Length > 65536) FlushIndex();
            return true;
        }

        // Appends the pending index lines.  A FileStream does not append atomically on Windows, so two
        // snapshots writing to one store would overwrite each other's lines: a named mutex serializes them.
        private void FlushIndex()
        {
            if (pending.Length == 0) return;
            pack.Flush();
            byte[] b = new UTF8Encoding(false).GetBytes(pending.ToString());
            // string.GetHashCode differs per process in .NET Core: use a fixed hash of the path
            uint h = 2166136261;
            foreach (char ch in dir.ToLowerInvariant()) { h = (h ^ ch) * 16777619; }
            string mutexName = "Global\\ibcmd_rs_blobstore_" + h.ToString("x8");
            using (System.Threading.Mutex m = new System.Threading.Mutex(false, mutexName))
            {
                bool owned = false;
                try { owned = m.WaitOne(60000); } catch (System.Threading.AbandonedMutexException) { owned = true; }
                try
                {
                    using (FileStream fs = new FileStream(indexPath, FileMode.OpenOrCreate, FileAccess.Write, FileShare.ReadWrite))
                    {
                        fs.Seek(0, SeekOrigin.End);
                        fs.Write(b, 0, b.Length);
                        fs.Flush();
                    }
                }
                finally { if (owned) m.ReleaseMutex(); }
            }
            pending.Length = 0;
        }

        public void Dispose()
        {
            FlushIndex();
            pack.Dispose();
        }
    }

    public static class Codec
    {
        private static readonly char[] HexDigits = "0123456789abcdef".ToCharArray();

        public static string HexLower(byte[] b)
        {
            if (b == null || b.Length == 0) return "";
            char[] c = new char[b.Length * 2];
            for (int i = 0; i < b.Length; i++) { c[i * 2] = HexDigits[b[i] >> 4]; c[i * 2 + 1] = HexDigits[b[i] & 15]; }
            return new string(c);
        }

        public static string Sha256(byte[] b)
        {
            using (SHA256 s = SHA256.Create()) { return HexLower(s.ComputeHash(b)); }
        }

        // empty | container | binary | v8text ({...} serialization) | text
        public static string Kind(byte[] b)
        {
            if (b == null || b.Length == 0) return "empty";
            int off = (b.Length >= 3 && b[0] == 0xEF && b[1] == 0xBB && b[2] == 0xBF) ? 3 : 0;
            if (b.Length >= 4 && b[0] == 0xFF && b[1] == 0xFF && b[2] == 0xFF && b[3] == 0x7F) return "container";
            int probe = Math.Min(b.Length, off + 4096);
            if (probe - off <= 0) return "empty";
            bool cutByWindow = off + 4096 < b.Length;
            // text = well-formed UTF-8 without control characters (a multi-byte character
            // cut by the end of the probe window is not an error)
            int i = off;
            while (i < probe)
            {
                byte x = b[i];
                if (x < 0x80)
                {
                    if (x == 0x7F || (x < 32 && x != 9 && x != 10 && x != 13)) return "binary";
                    i++;
                    continue;
                }
                int need = (x >= 0xC2 && x <= 0xDF) ? 1 : ((x >= 0xE0 && x <= 0xEF) ? 2 : ((x >= 0xF0 && x <= 0xF4) ? 3 : -1));
                if (need < 0) return "binary";
                if (i + need >= probe)
                {
                    if (cutByWindow) break;
                    return "binary";
                }
                byte lo = 0x80, hi = 0xBF;
                if (x == 0xE0) lo = 0xA0; else if (x == 0xED) hi = 0x9F; else if (x == 0xF0) lo = 0x90; else if (x == 0xF4) hi = 0x8F;
                if (b[i + 1] < lo || b[i + 1] > hi) return "binary";
                for (int k = 2; k <= need; k++)
                {
                    if ((b[i + k] & 0xC0) != 0x80) return "binary";
                }
                i += need + 1;
            }
            return b[off] == 0x7B ? "v8text" : "text";
        }

        public static string Preview(byte[] b, int maxChars)
        {
            int off = (b.Length >= 3 && b[0] == 0xEF && b[1] == 0xBB && b[2] == 0xBF) ? 3 : 0;
            int take = Math.Min(b.Length - off, maxChars * 3);
            string t = new UTF8Encoding(false).GetString(b, off, take);
            if (t.Length > maxChars) t = t.Substring(0, maxChars) + "...";
            StringBuilder sb = new StringBuilder(t.Length);
            bool space = false;
            foreach (char ch in t)
            {
                if (ch == '\r' || ch == '\n' || ch == '\t') { if (!space) sb.Append(' '); space = true; }
                else { sb.Append(ch); space = false; }
            }
            return sb.ToString();
        }

        // Strict raw-deflate inflate: null unless the whole input is exactly one deflate stream.
        public static byte[] Inflate(byte[] src, long maxOut)
        {
            if (src == null || src.Length < 2) return null;
            try
            {
                Puff p = new Puff(src, maxOut);
                p.Run();
                if (p.SrcPos != src.Length) return null;
                return p.Result();
            }
            catch (InvalidDataException) { return null; }
        }

        private sealed class Huffman
        {
            public short[] Count = new short[16];
            public short[] Symbol;
            public Huffman(int n) { Symbol = new short[n]; }
        }

        // A port of the classic "puff" decoder (public-domain algorithm by Mark Adler):
        // exact end-of-stream detection, so non-deflate bytes are not mistaken for deflate.
        private sealed class Puff
        {
            private readonly byte[] src;
            public int SrcPos;
            private int bitBuf, bitCnt;
            private byte[] outBuf = new byte[4096];
            private int outCount;
            private readonly long maxOut;

            public Puff(byte[] s, long max) { src = s; maxOut = max; }

            public byte[] Result() { byte[] r = new byte[outCount]; Buffer.BlockCopy(outBuf, 0, r, 0, outCount); return r; }

            private static InvalidDataException Bad(string m) { return new InvalidDataException(m); }

            private int Bits(int need)
            {
                int val = bitBuf;
                while (bitCnt < need)
                {
                    if (SrcPos >= src.Length) throw Bad("eof");
                    val |= ((int)src[SrcPos++]) << bitCnt;
                    bitCnt += 8;
                }
                bitBuf = val >> need;
                bitCnt -= need;
                return val & ((1 << need) - 1);
            }

            private void Put(byte b)
            {
                if (outCount >= maxOut) throw Bad("too big");
                if (outCount == outBuf.Length) Array.Resize(ref outBuf, outBuf.Length * 2);
                outBuf[outCount++] = b;
            }

            private int Decode(Huffman h)
            {
                int code = 0, first = 0, index = 0;
                for (int len = 1; len <= 15; len++)
                {
                    code |= Bits(1);
                    int count = h.Count[len];
                    if (code - count < first) return h.Symbol[index + (code - first)];
                    index += count; first += count; first <<= 1; code <<= 1;
                }
                throw Bad("bad code");
            }

            private static int Construct(Huffman h, short[] length, int off, int n)
            {
                for (int len = 0; len <= 15; len++) h.Count[len] = 0;
                for (int sym = 0; sym < n; sym++) h.Count[length[off + sym]]++;
                if (h.Count[0] == n) return 0;
                int left = 1;
                for (int len = 1; len <= 15; len++) { left <<= 1; left -= h.Count[len]; if (left < 0) return left; }
                short[] offs = new short[16];
                for (int len = 1; len < 15; len++) offs[len + 1] = (short)(offs[len] + h.Count[len]);
                for (int sym = 0; sym < n; sym++) if (length[off + sym] != 0) h.Symbol[offs[length[off + sym]]++] = (short)sym;
                return left;
            }

            private static readonly short[] Lens = { 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258 };
            private static readonly short[] LExt = { 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0 };
            private static readonly short[] Dists = { 1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577 };
            private static readonly short[] DExt = { 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13 };

            private void Codes(Huffman lencode, Huffman distcode)
            {
                int symbol;
                do
                {
                    symbol = Decode(lencode);
                    if (symbol < 256) Put((byte)symbol);
                    else if (symbol > 256)
                    {
                        symbol -= 257;
                        if (symbol >= 29) throw Bad("bad length symbol");
                        int len = Lens[symbol] + Bits(LExt[symbol]);
                        int ds = Decode(distcode);
                        if (ds >= 30) throw Bad("bad distance symbol");
                        int dist = Dists[ds] + Bits(DExt[ds]);
                        if (dist > outCount) throw Bad("distance too far");
                        while (len-- > 0) Put(outBuf[outCount - dist]);
                    }
                } while (symbol != 256);
            }

            private void Stored()
            {
                bitBuf = 0; bitCnt = 0;
                if (SrcPos + 4 > src.Length) throw Bad("eof");
                int len = src[SrcPos] | (src[SrcPos + 1] << 8);
                int nlen = src[SrcPos + 2] | (src[SrcPos + 3] << 8);
                SrcPos += 4;
                if (len != (~nlen & 0xFFFF)) throw Bad("stored length");
                if (SrcPos + len > src.Length) throw Bad("eof");
                for (int i = 0; i < len; i++) Put(src[SrcPos++]);
            }

            private void Fixed()
            {
                short[] lengths = new short[320];
                int sym = 0;
                for (; sym < 144; sym++) lengths[sym] = 8;
                for (; sym < 256; sym++) lengths[sym] = 9;
                for (; sym < 280; sym++) lengths[sym] = 7;
                for (; sym < 288; sym++) lengths[sym] = 8;
                Huffman lencode = new Huffman(288);
                Construct(lencode, lengths, 0, 288);
                for (sym = 0; sym < 30; sym++) lengths[sym] = 5;
                Huffman distcode = new Huffman(30);
                Construct(distcode, lengths, 0, 30);
                Codes(lencode, distcode);
            }

            private static readonly short[] Order = { 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 };

            private void Dynamic()
            {
                int nlen = Bits(5) + 257, ndist = Bits(5) + 1, ncode = Bits(4) + 4;
                if (nlen > 286 || ndist > 30) throw Bad("too many codes");
                short[] lengths = new short[320];
                int index;
                for (index = 0; index < ncode; index++) lengths[Order[index]] = (short)Bits(3);
                for (; index < 19; index++) lengths[Order[index]] = 0;
                Huffman lencode = new Huffman(286);
                if (Construct(lencode, lengths, 0, 19) != 0) throw Bad("incomplete code lengths code");
                index = 0;
                while (index < nlen + ndist)
                {
                    int symbol = Decode(lencode);
                    if (symbol < 16) lengths[index++] = (short)symbol;
                    else
                    {
                        int len = 0, rep;
                        if (symbol == 16) { if (index == 0) throw Bad("no last length"); len = lengths[index - 1]; rep = 3 + Bits(2); }
                        else if (symbol == 17) rep = 3 + Bits(3);
                        else rep = 11 + Bits(7);
                        if (index + rep > nlen + ndist) throw Bad("too many lengths");
                        while (rep-- > 0) lengths[index++] = (short)len;
                    }
                }
                if (lengths[256] == 0) throw Bad("no end-of-block code");
                int err = Construct(lencode, lengths, 0, nlen);
                if (err != 0 && (err < 0 || nlen != lencode.Count[0] + lencode.Count[1])) throw Bad("bad literal/length code");
                Huffman distcode = new Huffman(30);
                err = Construct(distcode, lengths, nlen, ndist);
                if (err != 0 && (err < 0 || ndist != distcode.Count[0] + distcode.Count[1])) throw Bad("bad distance code");
                Codes(lencode, distcode);
            }

            public void Run()
            {
                int last;
                do
                {
                    last = Bits(1);
                    int type = Bits(2);
                    if (type == 0) Stored();
                    else if (type == 1) Fixed();
                    else if (type == 2) Dynamic();
                    else throw Bad("bad block type");
                } while (last == 0);
            }
        }
    }
}
"@
}

# Upper-case hex (SQL Server style) of a byte array.
function Get-HexString {
    param([byte[]]$Bytes)
    if ($null -eq $Bytes -or $Bytes.Length -eq 0) { return '' }
    return [ApplyTraceKit.Codec]::HexLower($Bytes).ToUpperInvariant()
}

# SHA-256 of a byte array as lowercase hex.
function Get-Sha256Hex {
    param([byte[]]$Bytes)
    return [ApplyTraceKit.Codec]::Sha256($Bytes)
}

# Strict raw-deflate inflate (the storage format of most Config/Params rows):
# $null unless the bytes are exactly one deflate stream of at most $MaxBytes.
function Expand-RawDeflate {
    param([byte[]]$Bytes, [long]$MaxBytes = 268435456)
    $r = [ApplyTraceKit.Codec]::Inflate($Bytes, $MaxBytes)
    if ($null -eq $r) { return $null }
    return , $r
}

# empty | container | binary | v8text ({...} serialization) | text
function Get-ContentKind {
    param([byte[]]$Bytes)
    return [ApplyTraceKit.Codec]::Kind($Bytes)
}

# A single-line, bounded preview of decoded text bytes.
function Get-TextPreview {
    param([byte[]]$Bytes, [int]$MaxChars = 160)
    return [ApplyTraceKit.Codec]::Preview($Bytes, $MaxChars)
}

# Exports the events of Extended Events files to a gzip file: one record per event,
# "<timestamp_utc><TAB><event xml>", records separated by U+001E.  trace_report.py parses
# the XML (XQuery inside SQL Server needed minutes for 100k+ events).  Returns the count.
function Export-XelEvents {
    param($Conn, [string]$Pattern, [string]$OutPath)
    $cmd = $Conn.CreateCommand()
    $cmd.CommandText = "SELECT CONVERT(varchar(33), timestamp_utc, 126) AS ts, event_data FROM sys.fn_xe_file_target_read_file(N'$($Pattern.Replace("'", "''"))', NULL, NULL, NULL) ORDER BY file_name, file_offset"
    $cmd.CommandTimeout = 0
    $reader = $cmd.ExecuteReader()
    $fs = [System.IO.File]::Create($OutPath)
    $gz = New-Object System.IO.Compression.GZipStream($fs, [System.IO.Compression.CompressionLevel]::Fastest)
    $xw = New-Object System.IO.StreamWriter($gz, $script:Utf8NoBom)
    $n = 0
    try {
        while ($reader.Read()) {
            $xw.Write($reader.GetString(0))
            $xw.Write("`t")
            $xw.Write($reader.GetString(1))
            $xw.Write([char]0x1E)
            $n++
        }
    } finally { $reader.Close(); $xw.Dispose() }
    return $n
}

# Runs "the command under observation": a script block, or an executable with
# arguments.  Output goes to a log file.  The script block runs in the calling
# scope chain: prefer names that trace.ps1/capture.ps1 do not use as parameters.
#
# -TimeoutMinutes N: a watchdog on its own thread kills every process that
# descends from this PowerShell process N minutes after the start (exit code -9,
# TimedOut = true), so a hung native tool cannot stall a capture for hours.
#
# -Before / -After: script blocks run right before and after the command (After also when the command
# fails), outside its stopwatch and timeout: e.g. take and release a lab lock for the native command only.
function Invoke-Observed {
    param([scriptblock]$Command, [string]$Exe, [string[]]$ArgumentList, [string]$LogPath, [int]$TimeoutMinutes = 0,
          [scriptblock]$Before, [scriptblock]$After)
    if ($Before) { & $Before | Out-Host }
    $global:LASTEXITCODE = 0
    $code = 0
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $sync = [hashtable]::Synchronized(@{ Stop = $false; Fired = $false })
    $dog = $null
    if ($TimeoutMinutes -gt 0) {
        $dog = [powershell]::Create()
        [void]$dog.AddScript({
            param($ownPid, $deadline, $sync)
            function Get-Descendant($p) {
                Get-CimInstance Win32_Process -Filter "ParentProcessId=$p" -ErrorAction SilentlyContinue | ForEach-Object { $_.ProcessId; Get-Descendant $_.ProcessId }
            }
            while ((Get-Date) -lt $deadline) {
                if ($sync.Stop) { return }
                Start-Sleep -Milliseconds 500
            }
            if ($sync.Stop) { return }
            $sync.Fired = $true
            foreach ($d in @(Get-Descendant $ownPid)) { Stop-Process -Id $d -Force -ErrorAction SilentlyContinue }
        }).AddArgument($PID).AddArgument((Get-Date).AddMinutes($TimeoutMinutes)).AddArgument($sync)
        $dogHandle = $dog.BeginInvoke()
    }
    try {
        if ($Command) {
            & $Command *>&1 | Out-File -Encoding utf8 -LiteralPath $LogPath
            if ($null -ne $global:LASTEXITCODE) { $code = $global:LASTEXITCODE }
        } elseif ($Exe) {
            & $Exe @ArgumentList *>&1 | Out-File -Encoding utf8 -LiteralPath $LogPath
            $code = $LASTEXITCODE
        } else {
            throw 'give -Command <scriptblock> or -Exe <path> [-ArgumentList ...]'
        }
    } finally {
        $ErrorActionPreference = $prevPref
        $sw.Stop()
        $sync.Stop = $true
        if ($dog) { try { [void]$dog.Stop() } catch { } ; $dog.Dispose() }
        if ($After) { try { & $After | Out-Host } catch { Write-Log "the -After hook failed: $_" } }
    }
    if ($sync.Fired) { $code = -9; Write-Log "the command was killed after $TimeoutMinutes minutes (-TimeoutMinutes)" }
    return [pscustomobject]@{ ExitCode = $code; Seconds = [Math]::Round($sw.Elapsed.TotalSeconds, 1); TimedOut = [bool]$sync.Fired }
}
