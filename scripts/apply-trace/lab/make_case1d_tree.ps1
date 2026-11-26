# Case 1d (8.3.27): a tree of the 8.3.27 БСП reference export with ONE module edited (the common module of X2, edited again).
# Every unedited file is a HARD LINK to the reference file, so the tree costs no disk.
#   pwsh -NoProfile -File make_case1d_tree.ps1 [-Tree F:\ibcmd\lab\04\trace\tree1d]
param(
    [string]$Tree = 'F:\ibcmd\lab\04\trace\tree1d',
    [string]$Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class HardLinker2 {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool CreateHardLinkW(string newFile, string existingFile, System.IntPtr reserved);
}
'@
if (Test-Path -LiteralPath $Tree) { throw "$Tree exists already" }
$edits = @(
    @{ Kind = 'common module'; Path = 'CommonModules\_ДемоЛокализацияКлиентСервер\Ext\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1d: common module edited again`r`nФункция ТрассаСлучай1dОбщийМодуль() Экспорт`r`n`tВозврат `"case1d`";`r`nКонецФункции`r`n" }
)
$editSet = @{}
foreach ($e in $edits) { $editSet[$e.Path.ToLowerInvariant()] = $e }
$links = 0; $dirs = 0
function Walk($src, $dst, $rel) {
    New-Item -ItemType Directory -Force -Path $dst | Out-Null
    $script:dirs++
    foreach ($item in Get-ChildItem -LiteralPath $src -Force) {
        $r = if ($rel) { "$rel\$($item.Name)" } else { $item.Name }
        $target = Join-Path $dst $item.Name
        if ($item.PSIsContainer) { Walk $item.FullName $target $r; continue }
        if ($editSet.ContainsKey($r.ToLowerInvariant())) {
            $e = $editSet[$r.ToLowerInvariant()]
            $orig = [IO.File]::ReadAllBytes($item.FullName)
            [IO.File]::WriteAllBytes($target, $orig + [Text.Encoding]::UTF8.GetBytes($e.Add))
            continue
        }
        if (-not [HardLinker2]::CreateHardLinkW($target, $item.FullName, [IntPtr]::Zero)) { throw "hard link failed for $($item.FullName): $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
        $script:links++
    }
}
Walk $Ref $Tree ''
"tree $Tree : $dirs directories, $links hard links, $($edits.Count) edited file"
