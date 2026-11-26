# Edits five module texts of the copy of the reference tree (case 1: modules only).
# Byte-safe: keeps the BOM and the CRLF line breaks, appends at the end of the file.
param([string]$Tree = 'F:\ibcmd\lab\04\trace\tree\c1', [string]$Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native')
$ErrorActionPreference = 'Stop'
$edits = @(
    @{ Kind = 'common module'; Path = 'CommonModules\_ДемоЛокализацияКлиентСервер\Ext\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1: common module edited`r`nФункция ТрассаСлучай1ОбщийМодуль() Экспорт`r`n`tВозврат `"case1`";`r`nКонецФункции`r`n" },
    @{ Kind = 'catalog object module'; Path = 'Catalogs\_ДемоКонтрагенты\Ext\ObjectModule.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1: catalog object module edited`r`n" },
    @{ Kind = 'document manager module'; Path = 'Documents\_ДемоПоручениеЭкспедитору\Ext\ManagerModule.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1: document manager module edited`r`n" },
    @{ Kind = 'managed form module'; Path = 'Catalogs\_ДемоГруппыДоступаНоменклатуры\Forms\ФормаЭлемента\Ext\Form\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1: managed form module edited`r`n" },
    @{ Kind = 'common form module'; Path = 'CommonForms\ВопросОбУстановкеВнешнейКомпоненты\Ext\Form\Module.bsl'
       Add = "`r`n// ibcmd-rs trace, case 1: common form module edited`r`n" }
)
$manifest = @()
foreach ($e in $edits) {
    $f = Join-Path $Tree $e.Path
    $orig = [IO.File]::ReadAllBytes((Join-Path $Ref $e.Path))
    $add = [Text.Encoding]::UTF8.GetBytes($e.Add)
    [IO.File]::WriteAllBytes($f, $orig + $add)
    $manifest += ('{0}`t{1}`t{2} -> {3} bytes' -f $e.Kind, ($e.Path -replace '\\', '/'), $orig.Length, ($orig.Length + $add.Length))
}
$manifest | Set-Content -LiteralPath (Join-Path (Split-Path -Parent $Tree) ((Split-Path -Leaf $Tree) + '_edits.txt')) -Encoding UTF8
$manifest
