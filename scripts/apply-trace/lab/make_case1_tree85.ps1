# Case 1 on 8.5 (modules only): a tree of the 8.5 БСП reference export in which five module texts are edited.
# The tree costs no disk: every unedited file is a HARD LINK to the reference file (a junction would not be followed by
# a directory walker), the five edited files are real copies with the edit appended (BOM and CRLF kept).
#   pwsh -NoProfile -File make_case1_tree85.ps1 [-Tree F:\ibcmd\lab\04\trace\tree85]
param(
    [string]$Tree = 'F:\ibcmd\lab\04\trace\tree85',
    [string]$Ref = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native'
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class HardLinker {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool CreateHardLinkW(string newFile, string existingFile, System.IntPtr reserved);
}
'@
if (Test-Path -LiteralPath $Tree) { throw "$Tree exists already" }
$edits = @(
    @{ Kind = 'common module'; Path = 'CommonModules\_ДемоЛокализацияКлиентСервер\Ext\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1 (8.5): common module edited`r`nФункция ТрассаСлучай1ОбщийМодуль() Экспорт`r`n`tВозврат `"case1`";`r`nКонецФункции`r`n" },
    @{ Kind = 'catalog object module'; Path = 'Catalogs\_ДемоКонтрагенты\Ext\ObjectModule.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1 (8.5): catalog object module edited`r`n" },
    @{ Kind = 'document manager module'; Path = 'Documents\_ДемоПоручениеЭкспедитору\Ext\ManagerModule.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1 (8.5): document manager module edited`r`n" },
    @{ Kind = 'managed form module'; Path = 'Catalogs\_ДемоДоговорыКонтрагентов\Forms\ФормаЭлемента\Ext\Form\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1 (8.5): managed form module edited`r`n" },
    @{ Kind = 'common form module'; Path = 'CommonForms\ВопросОбУстановкеВнешнейКомпоненты\Ext\Form\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1 (8.5): common form module edited`r`n" }
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
            $add = [Text.Encoding]::UTF8.GetBytes($e.Add)
            [IO.File]::WriteAllBytes($target, $orig + $add)
            $script:manifest += ('{0}`t{1}`t{2} -> {3} bytes' -f $e.Kind, ($r -replace '\\', '/'), $orig.Length, ($orig.Length + $add.Length))
            continue
        }
        if (-not [HardLinker]::CreateHardLinkW($target, $item.FullName, [IntPtr]::Zero)) { throw "hard link failed for $($item.FullName): $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
        $script:links++
    }
}
$manifest = @()
Walk $Ref $Tree ''
$manifest | Set-Content -LiteralPath (Join-Path (Split-Path -Parent $Tree) ((Split-Path -Leaf $Tree) + '_edits.txt')) -Encoding UTF8
"tree $Tree : $dirs directories, $links hard links, $($manifest.Count) edited files"
$manifest
