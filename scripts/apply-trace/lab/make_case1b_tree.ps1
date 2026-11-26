# Case 1b: a second, small edit of the same five module files of tree\c1 (steady-state apply on a cleaned infobase).
param([string]$Tree = 'F:\ibcmd\lab\04\trace\tree\c1')
$ErrorActionPreference = 'Stop'
$files = @(
    'CommonModules\_ДемоЛокализацияКлиентСервер\Ext\Module.bsl',
    'Catalogs\_ДемоКонтрагенты\Ext\ObjectModule.bsl',
    'Documents\_ДемоПоручениеЭкспедитору\Ext\ManagerModule.bsl',
    'Catalogs\_ДемоГруппыДоступаНоменклатуры\Forms\ФормаЭлемента\Ext\Form\Module.bsl',
    'CommonForms\ВопросОбУстановкеВнешнейКомпоненты\Ext\Form\Module.bsl')
foreach ($f in $files) {
    $p = Join-Path $Tree $f
    $b = [IO.File]::ReadAllBytes($p)
    $add = [Text.Encoding]::UTF8.GetBytes("// ibcmd-rs trace, case 1b: second edit`r`n")
    [IO.File]::WriteAllBytes($p, $b + $add)
    '{0}: {1} -> {2} bytes' -f $f, $b.Length, ($b.Length + $add.Length)
}
