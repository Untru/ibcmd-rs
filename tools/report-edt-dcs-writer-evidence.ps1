<#
.SYNOPSIS
Extracts a portable, fail-closed DCS writer evidence report from an installed EDT.

.DESCRIPTION
This is a research-only extractor. InputInventory is an EXTERNAL, version-matched
inventory whose selected bundle entries contain existing local JAR paths. The
checked-in model inventory is not a valid input because it intentionally contains
no reusable binary locations.

The extractor retains no JAR, class, bytecode, Xcore, source, or machine path.
Only parser-proven facts and portable edt-derived:// coordinates reach the report.

.EXAMPLE
pwsh tools/report-edt-dcs-writer-evidence.ps1 `
  -InputInventory <external-edt-2025.2.3+30-inventory.json> `
  -EdtRelease 2025.2.3+30 `
  -OutputReport crates/ibcmd-schema/data/edt-2025.2.3-dcs-writer-evidence.json

.EXAMPLE
pwsh tools/report-edt-dcs-writer-evidence.ps1 -SelfTest
#>
[CmdletBinding(DefaultParameterSetName = 'Extract')]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$InputInventory,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$OutputReport,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$EdtRelease,

    [Parameter(Mandatory = $true, ParameterSetName = 'SelfTest')]
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$dcsBundleName = 'com._1c.g5.v8.dt.dcs'
$formBundleName = 'com._1c.g5.v8.dt.form.export.xml'
$serializerClass = 'com._1c.g5.v8.dt.dcs.util.DcsV8Serializer'
$listSettingsClass = 'com._1c.g5.v8.dt.form.export.xml.writer.ListSettingsWriter'

function Invoke-EdtJavap {
    param(
        [Parameter(Mandatory)] [string]$Jar,
        [Parameter(Mandatory)] [string]$ClassName
    )

    $lines = @(& javap -classpath $Jar -v -p -c -constants $ClassName 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "javap failed for ${ClassName}: $($lines -join "`n")"
    }
    return $lines
}

function Get-JavapMethodBlock {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] [string]$HeaderPattern
    )

    $starts = @()
    for ($index = 0; $index -lt $Lines.Count; $index++) {
        if ($Lines[$index] -match $HeaderPattern) {
            $starts += $index
        }
    }
    if ($starts.Count -ne 1) {
        throw "Expected exactly one javap method matching '$HeaderPattern'; found $($starts.Count)."
    }

    $start = $starts[0]
    $end = $Lines.Count
    for ($index = $start + 1; $index -lt $Lines.Count; $index++) {
        if ($Lines[$index] -match '^  (?:public |private |protected )?(?:static )?.+\);$' -or
            $Lines[$index] -match '^  static \{\};$') {
            $end = $index
            break
        }
    }
    return @($Lines[$start..($end - 1)])
}

function ConvertTo-JavapInstructions {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $result = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $Lines) {
        if ($line -notmatch '^\s+(\d+):\s+([a-z][a-z0-9_]*)\s*(.*)$') {
            continue
        }
        $tail = [string]$Matches[3]
        $operand = $tail
        $comment = ''
        $commentIndex = $tail.IndexOf('//')
        if ($commentIndex -ge 0) {
            $operand = $tail.Substring(0, $commentIndex)
            $comment = $tail.Substring($commentIndex + 2)
        }
        $result.Add([ordered]@{
            offset = [int]$Matches[1]
            opcode = [string]$Matches[2]
            operand = $operand.Trim()
            comment = $comment.Trim()
            text = $line.Trim()
        })
    }
    return @($result)
}

function Get-JavapConstantPool {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $pool = @{}
    foreach ($line in $Lines) {
        if ($line -notmatch '^\s+#(\d+) = ([A-Za-z]+)\s+(.+)$') {
            continue
        }
        $index = [int]$Matches[1]
        $kind = [string]$Matches[2]
        $tail = [string]$Matches[3]
        $separator = $tail.IndexOf('//')
        $target = if ($separator -ge 0) { $tail.Substring($separator + 2).Trim() } else { '' }
        if ($pool.ContainsKey($index)) {
            throw "Duplicate constant-pool index #$index."
        }
        $pool[$index] = [ordered]@{
            kind = $kind
            target = $target
            text = $line.Trim()
        }
    }
    if ($pool.Count -eq 0) {
        throw 'Verbose javap output has no parsed constant pool.'
    }
    return $pool
}

function Get-InstructionAtOffset {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] [int]$Offset
    )

    $matches = @($Instructions | Where-Object { [int]$_['offset'] -eq $Offset })
    if ($matches.Count -ne 1) {
        throw "Expected exactly one instruction at offset $Offset; found $($matches.Count)."
    }
    return $matches[0]
}

function Assert-Instruction {
    param(
        [Parameter(Mandatory)] $Instruction,
        [Parameter(Mandatory)] [string]$Opcode,
        [string]$Operand
    )

    if ([string]$Instruction['opcode'] -ne $Opcode) {
        throw "Expected opcode '$Opcode', found '$($Instruction['text'])'."
    }
    if ($PSBoundParameters.ContainsKey('Operand') -and
        [string]$Instruction['operand'] -ne $Operand) {
        throw "Expected operand '$Operand', found '$($Instruction['text'])'."
    }
}

function Get-ConstantPoolIndex {
    param([Parameter(Mandatory)] $Instruction)

    if ([string]$Instruction['operand'] -notmatch '^#(\d+)(?:,.*)?$') {
        throw "Instruction has no single constant-pool operand: '$($Instruction['text'])'."
    }
    return [int]$Matches[1]
}

function Get-PortableMemberName {
    param([Parameter(Mandatory)] [string]$Target)

    if ($Target -notmatch '^.+/([^/.]+)\.([^.:]+):') {
        throw "Constant-pool target '$Target' has no portable class/member coordinate."
    }
    return "$($Matches[1]).$($Matches[2])"
}

function Assert-CpInstruction {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] $ConstantPool,
        [Parameter(Mandatory)] [int]$Offset,
        [Parameter(Mandatory)] [string]$Opcode,
        [Parameter(Mandatory)] [string]$Kind,
        [Parameter(Mandatory)] [AllowEmptyString()] [string]$TargetPattern
    )

    $instruction = Get-InstructionAtOffset -Instructions $Instructions -Offset $Offset
    Assert-Instruction -Instruction $instruction -Opcode $Opcode
    $index = Get-ConstantPoolIndex -Instruction $instruction
    if (-not $ConstantPool.ContainsKey($index)) {
        throw "Instruction at offset $Offset refers to absent constant-pool entry #$index."
    }
    $entry = $ConstantPool[$index]
    if ([string]$entry['kind'] -ne $Kind) {
        throw "Constant-pool entry #$index has kind '$($entry['kind'])', expected '$Kind'."
    }
    if ([string]$entry['target'] -notmatch $TargetPattern) {
        throw "Constant-pool entry #$index target '$($entry['target'])' does not match '$TargetPattern'."
    }
    return [string]$entry['target']
}

function Assert-ExactSubsequence {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] [int]$StartOffset,
        [Parameter(Mandatory)] [int[]]$Offsets,
        [Parameter(Mandatory)] [string[]]$Opcodes
    )

    if ($Offsets.Count -ne $Opcodes.Count) {
        throw 'Internal extractor error: offset/opcode shape lengths differ.'
    }
    $startIndex = -1
    for ($index = 0; $index -lt $Instructions.Count; $index++) {
        if ([int]$Instructions[$index]['offset'] -eq $StartOffset) {
            $startIndex = $index
            break
        }
    }
    if ($startIndex -lt 0 -or $startIndex + $Offsets.Count -gt $Instructions.Count) {
        throw "Instruction sequence beginning at $StartOffset is absent or truncated."
    }
    for ($index = 0; $index -lt $Offsets.Count; $index++) {
        $instruction = $Instructions[$startIndex + $index]
        if ([int]$instruction['offset'] -ne $Offsets[$index]) {
            throw "Expected instruction offset $($Offsets[$index]), found '$($instruction['text'])'."
        }
        Assert-Instruction -Instruction $instruction -Opcode $Opcodes[$index]
    }
}

function Assert-SettingsMethodEnvelope {
    param([Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions)

    if ($Instructions.Count -ne 171) {
        throw "DCS writeSettings instruction count is $($Instructions.Count), expected 171."
    }
    if (@($Instructions | Where-Object { [string]$_['opcode'] -eq 'invokedynamic' }).Count -ne 0) {
        throw 'DCS writeSettings unexpectedly contains invokedynamic; bootstrap resolution is unsupported.'
    }
    $branches = @($Instructions | Where-Object {
        [string]$_['opcode'] -match '^(?:if[a-z_]*|goto|tableswitch|lookupswitch|jsr)$'
    } | ForEach-Object {
        "$($_['offset']):$($_['opcode']):$($_['operand'])"
    })
    $expected = @(
        '1:ifnonnull:5',
        '9:ifeq:25',
        '22:goto:392',
        '219:goto:257',
        '264:ifne:222',
        '273:ifnull:314'
    )
    if ([string]::Join('|', $branches) -ne [string]::Join('|', $expected)) {
        throw "DCS writeSettings branch graph is unknown: $([string]::Join('|', $branches))."
    }
    $last = $Instructions[$Instructions.Count - 1]
    Assert-Instruction -Instruction $last -Opcode 'return'
    if ([int]$last['offset'] -ne 392) {
        throw "DCS writeSettings final return is at offset $($last['offset']), expected 392."
    }
}

function Get-SettingsDefaultFact {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] $ConstantPool
    )

    Assert-ExactSubsequence -Instructions $Instructions -StartOffset 0 `
        -Offsets @(0, 1, 4, 5, 6, 9, 12, 13, 14, 17, 18, 19, 22, 25, 26, 27, 30, 31, 32) `
        -Opcodes @('aload_2', 'ifnonnull', 'return', 'aload_2', 'invokestatic', 'ifeq',
            'aload_1', 'aload_3', 'invokestatic', 'aload_0', 'aload_1', 'invokevirtual',
            'goto', 'aload_1', 'aload_3', 'invokestatic', 'aload_0', 'aload_1', 'invokevirtual')
    Assert-Instruction -Instruction (Get-InstructionAtOffset $Instructions 1) -Opcode 'ifnonnull' -Operand '5'
    Assert-Instruction -Instruction (Get-InstructionAtOffset $Instructions 9) -Opcode 'ifeq' -Operand '25'
    Assert-Instruction -Instruction (Get-InstructionAtOffset $Instructions 22) -Opcode 'goto' -Operand '392'
    $predicate = Assert-CpInstruction $Instructions $ConstantPool 6 'invokestatic' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/DcsDefaultValueUtil\.isDefaultValue:'
    $emptyWriter = Assert-CpInstruction $Instructions $ConstantPool 14 'invokestatic' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer\.writeEmptyElement:'
    $namespaceWriter = Assert-CpInstruction $Instructions $ConstantPool 19 'invokevirtual' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/DcsV8Serializer\.writeSettingsNamespace:'
    $null = Assert-CpInstruction $Instructions $ConstantPool 27 'invokestatic' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer\.writeStartElement:'
    $null = Assert-CpInstruction $Instructions $ConstantPool 32 'invokevirtual' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/DcsV8Serializer\.writeSettingsNamespace:'

    return [ordered]@{
        predicate = Get-PortableMemberName $predicate
        operations = @(
            Get-PortableMemberName $emptyWriter
            Get-PortableMemberName $namespaceWriter
        )
    }
}

function Get-SettingsTailFacts {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] $ConstantPool
    )

    $offsets = @(314, 315, 316, 321, 324, 327, 328, 330, 333, 336,
        339, 340, 341, 346, 349, 352, 353, 355, 358, 361)
    $opcodes = @('aload_1', 'aload_2', 'invokeinterface', 'getstatic', 'new', 'dup',
        'ldc', 'ldc_w', 'invokespecial', 'invokestatic', 'aload_1', 'aload_2',
        'invokeinterface', 'ldc_w', 'new', 'dup', 'ldc', 'ldc_w', 'invokespecial',
        'invokestatic')
    Assert-ExactSubsequence $Instructions 314 $offsets $opcodes

    $viewAccessor = Assert-CpInstruction $Instructions $ConstantPool 316 'invokeinterface' `
        'InterfaceMethodref' 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettings\.getItemsViewMode:'
    $viewDefault = Assert-CpInstruction $Instructions $ConstantPool 321 'getstatic' `
        'Fieldref' 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettingsItemViewMode\.QUICK_ACCESS:'
    $null = Assert-CpInstruction $Instructions $ConstantPool 324 'new' 'Class' 'javax/xml/namespace/QName$'
    $namespace = Assert-CpInstruction $Instructions $ConstantPool 328 'ldc' 'String' `
        'http://v8\.1c\.ru/8\.1/data-composition-system/settings$'
    $viewName = Assert-CpInstruction $Instructions $ConstantPool 330 'ldc_w' 'String' 'itemsViewMode$'
    $null = Assert-CpInstruction $Instructions $ConstantPool 333 'invokespecial' 'Methodref' `
        'javax/xml/namespace/QName\."<init>":\(Ljava/lang/String;Ljava/lang/String;\)V$'
    $viewWriter = Assert-CpInstruction $Instructions $ConstantPool 336 'invokestatic' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer\.writeEnumNotDefault:'
    $idAccessor = Assert-CpInstruction $Instructions $ConstantPool 341 'invokeinterface' `
        'InterfaceMethodref' 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettings\.getItemsUserSettingID:'
    $idDefault = Assert-CpInstruction $Instructions $ConstantPool 346 'ldc_w' 'String' '^$'
    $null = Assert-CpInstruction $Instructions $ConstantPool 349 'new' 'Class' 'javax/xml/namespace/QName$'
    $idNamespace = Assert-CpInstruction $Instructions $ConstantPool 353 'ldc' 'String' `
        'http://v8\.1c\.ru/8\.1/data-composition-system/settings$'
    $idName = Assert-CpInstruction $Instructions $ConstantPool 355 'ldc_w' 'String' 'itemsUserSettingID$'
    $null = Assert-CpInstruction $Instructions $ConstantPool 358 'invokespecial' 'Methodref' `
        'javax/xml/namespace/QName\."<init>":\(Ljava/lang/String;Ljava/lang/String;\)V$'
    $idWriter = Assert-CpInstruction $Instructions $ConstantPool 361 'invokestatic' 'Methodref' `
        'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer\.writeStringNotDefault:'

    if ($namespace -ne $idNamespace) {
        throw 'DCS settings tail uses different namespaces for its two verified fields.'
    }
    if ($viewDefault -notmatch '\.([A-Z0-9_]+):') {
        throw 'DCS settings tail default is not a literal enum constant.'
    }
    $viewDefaultConstant = [string]$Matches[1]
    if ($viewAccessor -notmatch '\.getItemsViewMode:' -or
        $viewWriter -notmatch '\.writeEnumNotDefault:' -or
        $idAccessor -notmatch '\.getItemsUserSettingID:' -or $idDefault -ne '' -or
        $idWriter -notmatch '\.writeStringNotDefault:') {
        throw 'DCS settings tail does not have the accepted accessor/default/writer relationships.'
    }

    return [ordered]@{
        namespace = $namespace
        order = @($viewName, $idName)
        itemsViewMode = [ordered]@{
            qname = "{$namespace}$viewName"
            defaultModelConstant = $viewDefaultConstant
            writer = Get-PortableMemberName $viewWriter
        }
        itemsUserSettingID = [ordered]@{
            qname = "{$namespace}$idName"
            defaultString = $idDefault
            writer = Get-PortableMemberName $idWriter
        }
    }
}

function Get-ListSettingsFact {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] $ConstantPool
    )

    $offsets = @(0, 1, 4, 7, 10, 11, 13, 14, 17, 18, 19, 20, 23, 26, 27, 30,
        33, 34, 35, 36, 41, 44, 46, 48, 51, 52, 55, 57, 62, 64, 67, 68, 69, 72,
        75, 77, 79, 82, 83, 84, 86, 89, 91, 92, 95, 96, 101, 103, 106)
    $opcodes = @('aload_3', 'getstatic', 'if_acmpeq', 'new', 'dup', 'ldc', 'iconst_1',
        'anewarray', 'dup', 'iconst_0', 'aload_0', 'invokevirtual', 'invokevirtual',
        'aastore', 'invokestatic', 'invokespecial', 'athrow', 'aload_2', 'aload_3',
        'invokeinterface', 'checkcast', 'astore', 'aload', 'ifnull', 'aload_0',
        'getfield', 'aload', 'invokeinterface', 'astore', 'new', 'dup', 'aload_0',
        'getfield', 'invokespecial', 'astore', 'aload', 'new', 'dup', 'aload_1',
        'aload', 'invokespecial', 'aload', 'aload_0', 'getfield', 'aload_3',
        'invokeinterface', 'aload', 'invokevirtual', 'return')
    if ($Instructions.Count -ne $offsets.Count) {
        throw "ListSettingsWriter.write instruction count is $($Instructions.Count), expected $($offsets.Count)."
    }
    Assert-ExactSubsequence $Instructions 0 $offsets $opcodes
    if (@($Instructions | Where-Object { [string]$_['opcode'] -eq 'invokedynamic' }).Count -ne 0) {
        throw 'ListSettingsWriter.write unexpectedly contains invokedynamic.'
    }
    Assert-Instruction (Get-InstructionAtOffset $Instructions 4) 'if_acmpeq' '34'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 48) 'ifnull' '106'
    $null = Assert-CpInstruction $Instructions $ConstantPool 1 'getstatic' 'Fieldref' `
        'FormPackage\$Literals\.DYNAMIC_LIST_EXT_INFO__LIST_SETTINGS:'
    $null = Assert-CpInstruction $Instructions $ConstantPool 41 'checkcast' 'Class' `
        'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettings$'
    $provider = Assert-CpInstruction $Instructions $ConstantPool 96 'invokeinterface' `
        'InterfaceMethodref' 'com/_1c/g5/v8/dt/export/xml/IQNameProvider\.getElementQName:'
    $delegate = Assert-CpInstruction $Instructions $ConstantPool 103 'invokevirtual' `
        'Methodref' 'com/_1c/g5/v8/dt/dcs/util/DcsV8Serializer\.writeSettings:'
    if ($provider -notmatch '\.getElementQName:' -or $delegate -notmatch '\.writeSettings:') {
        throw 'ListSettingsWriter provider/delegate relationship is not accepted.'
    }
    $nullBranch = Get-InstructionAtOffset $Instructions 48
    $nullTarget = [int]$nullBranch['operand']
    $nullTargetInstruction = Get-InstructionAtOffset $Instructions $nullTarget
    return [ordered]@{
        delegate = Get-PortableMemberName $delegate
        qnameSource = Get-PortableMemberName $provider
        nullBranch = [ordered]@{
            fromOffset = [int]$nullBranch['offset']
            targetOffset = $nullTarget
            targetOpcode = [string]$nullTargetInstruction['opcode']
        }
    }
}

function New-Evidence {
    param(
        [Parameter(Mandatory)] [string[]]$Sources,
        [Parameter(Mandatory)] [string]$Note
    )
    return [ordered]@{
        kind = 'javap-v-exact-method-control-flow-constant-pool'
        status = 'verified'
        sources = @($Sources)
        note = $Note
    }
}

function ConvertTo-DeterministicJson {
    param([Parameter(Mandatory)] $Value)
    return (($Value | ConvertTo-Json -Depth 16 -Compress).Replace("`r`n", "`n") + "`n")
}

function Write-Utf8LfFile {
    param(
        [Parameter(Mandatory)] [string]$Path,
        [Parameter(Mandatory)] [string]$Text
    )
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($fullPath)) | Out-Null
    [System.IO.File]::WriteAllText($fullPath, $Text, [System.Text.UTF8Encoding]::new($false))
}

function New-TestInstruction {
    param([int]$Offset, [string]$Opcode, [string]$Operand = '', [string]$Comment = '')
    return [ordered]@{
        offset = $Offset
        opcode = $Opcode
        operand = $Operand
        comment = $Comment
        text = "$Offset`: $Opcode $Operand"
    }
}

function New-TestCpEntry {
    param([string]$Kind, [AllowEmptyString()] [string]$Target)
    return [ordered]@{ kind = $Kind; target = $Target; text = "$Kind // $Target" }
}

function Assert-SelfTestRejected {
    param([Parameter(Mandatory)] [scriptblock]$Action, [Parameter(Mandatory)] [string]$Name)
    $rejected = $false
    try {
        & $Action | Out-Null
    }
    catch {
        $rejected = $true
    }
    if (-not $rejected) {
        throw "Self-test '$Name' was not rejected."
    }
}

function Invoke-SelfTest {
    $tailOffsets = @(314, 315, 316, 321, 324, 327, 328, 330, 333, 336,
        339, 340, 341, 346, 349, 352, 353, 355, 358, 361)
    $tailOpcodes = @('aload_1', 'aload_2', 'invokeinterface', 'getstatic', 'new', 'dup',
        'ldc', 'ldc_w', 'invokespecial', 'invokestatic', 'aload_1', 'aload_2',
        'invokeinterface', 'ldc_w', 'new', 'dup', 'ldc', 'ldc_w', 'invokespecial',
        'invokestatic')
    $tailCp = @{
        316 = @(1, 'InterfaceMethodref', 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettings.getItemsViewMode:()V')
        321 = @(2, 'Fieldref', 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettingsItemViewMode.QUICK_ACCESS:Lx;')
        324 = @(3, 'Class', 'javax/xml/namespace/QName')
        328 = @(4, 'String', 'http://v8.1c.ru/8.1/data-composition-system/settings')
        330 = @(5, 'String', 'itemsViewMode')
        333 = @(6, 'Methodref', 'javax/xml/namespace/QName."<init>":(Ljava/lang/String;Ljava/lang/String;)V')
        336 = @(7, 'Methodref', 'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer.writeEnumNotDefault:(Lx;)V')
        341 = @(8, 'InterfaceMethodref', 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettings.getItemsUserSettingID:()V')
        346 = @(9, 'String', '')
        349 = @(3, 'Class', 'javax/xml/namespace/QName')
        353 = @(4, 'String', 'http://v8.1c.ru/8.1/data-composition-system/settings')
        355 = @(10, 'String', 'itemsUserSettingID')
        358 = @(6, 'Methodref', 'javax/xml/namespace/QName."<init>":(Ljava/lang/String;Ljava/lang/String;)V')
        361 = @(11, 'Methodref', 'com/_1c/g5/v8/dt/dcs/util/V8XmlSerializer.writeStringNotDefault:(Lx;)V')
    }
    $pool = @{}
    $instructions = [System.Collections.Generic.List[object]]::new()
    for ($index = 0; $index -lt $tailOffsets.Count; $index++) {
        $offset = $tailOffsets[$index]
        if ($tailCp.ContainsKey($offset)) {
            $cp = $tailCp[$offset]
            $pool[[int]$cp[0]] = New-TestCpEntry $cp[1] $cp[2]
            $commentPrefix = switch ($cp[1]) {
                'Fieldref' { 'Field ' }
                'Methodref' { 'Method ' }
                'InterfaceMethodref' { 'InterfaceMethod ' }
                'Class' { 'class ' }
                'String' { 'String ' }
            }
            $instructions.Add((New-TestInstruction $offset $tailOpcodes[$index] "#$($cp[0])" "$commentPrefix$($cp[2])"))
        }
        else {
            $instructions.Add((New-TestInstruction $offset $tailOpcodes[$index]))
        }
    }
    $null = Get-SettingsTailFacts @($instructions) $pool

    $badOrderPool = $pool.Clone()
    $badOrderPool[5] = New-TestCpEntry 'String' 'itemsUserSettingID'
    Assert-SelfTestRejected { Get-SettingsTailFacts @($instructions) $badOrderPool } 'perturbed-tail-order'
    $badDefaultPool = $pool.Clone()
    $badDefaultPool[2] = New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/dcs/model/settings/DataCompositionSettingsItemViewMode.COMPACT:Lx;'
    Assert-SelfTestRejected { Get-SettingsTailFacts @($instructions) $badDefaultPool } 'perturbed-default'

    $branchInstructions = @(
        New-TestInstruction 0 'aload_2',
        New-TestInstruction 1 'ifnonnull' '6',
        New-TestInstruction 4 'return'
    )
    Assert-SelfTestRejected {
        Assert-Instruction (Get-InstructionAtOffset $branchInstructions 1) 'ifnonnull' '5'
    } 'perturbed-branch'

    $providerPool = @{
        1 = New-TestCpEntry 'InterfaceMethodref' 'com/example/FixedQNameProvider.getElementQName:(Lx;)Ly;'
    }
    $providerInstruction = @(
        New-TestInstruction 96 'invokeinterface' '#1,  2' 'InterfaceMethod com/example/FixedQNameProvider.getElementQName:(Lx;)Ly;'
    )
    Assert-SelfTestRejected {
        Assert-CpInstruction $providerInstruction $providerPool 96 'invokeinterface' `
            'InterfaceMethodref' 'com/_1c/g5/v8/dt/export/xml/IQNameProvider\.getElementQName:'
    } 'perturbed-qname-provider'

    $delegatePool = @{
        1 = New-TestCpEntry 'Methodref' 'com/example/OtherSerializer.writeSettings:(Lx;)V'
    }
    $delegateInstruction = @(
        New-TestInstruction 103 'invokevirtual' '#1' 'Method com/example/OtherSerializer.writeSettings:(Lx;)V'
    )
    Assert-SelfTestRejected {
        Assert-CpInstruction $delegateInstruction $delegatePool 103 'invokevirtual' `
            'Methodref' 'com/_1c/g5/v8/dt/dcs/util/DcsV8Serializer\.writeSettings:'
    } 'perturbed-delegate'

    Write-Output 'DCS writer evidence extractor self-tests passed.'
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if (-not (Get-Command javap -ErrorAction SilentlyContinue)) {
    throw 'javap is required for EDT DCS writer research extraction.'
}
if ($EdtRelease -notmatch '^\d{4}\.\d+\.\d+\+\d+$') {
    throw "EdtRelease '$EdtRelease' is not an exact EDT release identifier."
}

$inventoryDocument = Get-Content -LiteralPath $InputInventory -Raw | ConvertFrom-Json
$inventory = @($inventoryDocument | ForEach-Object { $_ })
if ($inventory.Count -eq 0 -or @($inventory | Where-Object {
    $null -eq $_.PSObject.Properties['bundle'] -or
    $null -eq $_.PSObject.Properties['jar']
}).Count -ne 0) {
    throw 'External research inventory must be a top-level bundle array with bundle and local jar fields.'
}
function Get-ResearchBundle {
    param([Parameter(Mandatory)] [string]$Name)
    $entries = @($inventory | Where-Object { [string]$_.bundle -eq $Name })
    if ($entries.Count -ne 1) {
        throw "External research inventory must contain exactly one '$Name' entry; found $($entries.Count)."
    }
    $jar = [string]$entries[0].jar
    if ([string]::IsNullOrWhiteSpace($jar) -or -not (Test-Path -LiteralPath $jar -PathType Leaf)) {
        throw "External research inventory entry '$Name' must provide an existing version-matched jar path."
    }
    return $entries[0]
}

$dcsBundle = Get-ResearchBundle $dcsBundleName
$formBundle = Get-ResearchBundle $formBundleName
$serializerLines = @(Invoke-EdtJavap ([string]$dcsBundle.jar) $serializerClass)
$formLines = @(Invoke-EdtJavap ([string]$formBundle.jar) $listSettingsClass)
$serializerPool = Get-JavapConstantPool $serializerLines
$formPool = Get-JavapConstantPool $formLines
$settingsBlock = Get-JavapMethodBlock $serializerLines (
    '^  private void writeSettings\(com\._1c\.g5\.v8\.dt\.export\.xml\.writer\.ExportContextXmlStreamWriter, ' +
    'com\._1c\.g5\.v8\.dt\.dcs\.model\.settings\.DataCompositionSettings, javax\.xml\.namespace\.QName, ' +
    'com\._1c\.g5\.v8\.dt\.platform\.version\.Version, java\.util\.Map<java\.lang\.String, java\.util\.UUID>\)')
$listSettingsBlock = Get-JavapMethodBlock $formLines '^  public void write\('
$settingsInstructions = @(ConvertTo-JavapInstructions $settingsBlock)
$listSettingsInstructions = @(ConvertTo-JavapInstructions $listSettingsBlock)

Assert-SettingsMethodEnvelope $settingsInstructions
$defaultFact = Get-SettingsDefaultFact $settingsInstructions $serializerPool
$tailFacts = Get-SettingsTailFacts $settingsInstructions $serializerPool
$listSettingsFact = Get-ListSettingsFact $listSettingsInstructions $formPool

$dcsSource = "edt-derived://$EdtRelease/$dcsBundleName/$serializerClass#writeSettings"
$formSource = "edt-derived://$EdtRelease/$formBundleName/$listSettingsClass#write"
$settingsEvidence = New-Evidence @(
    'tools/report-edt-dcs-writer-evidence.ps1',
    $dcsSource
) 'Exact 171-instruction method envelope, complete branch graph, relevant opcode subsequences, and referenced constant-pool entries were parsed fail-closed.'
$formEvidence = New-Evidence @(
    'tools/report-edt-dcs-writer-evidence.ps1',
    $formSource
) 'Exact 49-instruction method, branch targets, QName-provider call, and DCS delegation constant-pool entries were parsed fail-closed.'

$report = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = '1C:EDT'
        release = $EdtRelease
        derivation = 'research-only external version-matched inventory; javap -v -p -c -constants exact method/control-flow/constant-pool extraction; no JAR, bytecode, source, Xcore, or machine path retained'
        inputContract = 'external inventory entries for the exact release must contain existing local JAR paths for the named bundles; the checked-in model inventory is intentionally insufficient'
        invocation = 'pwsh tools/report-edt-dcs-writer-evidence.ps1 -InputInventory <external-version-matched-inventory.json> -EdtRelease <release> -OutputReport <portable-report.json>'
    }
    verifiedFacts = @(
        [ordered]@{
            key = 'dcs.DataCompositionSettings.namespace'
            value = $tailFacts['namespace']
            evidence = $settingsEvidence
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.verified-tail-order'
            value = @($tailFacts['order'])
            evidence = $settingsEvidence
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.itemsViewMode'
            value = $tailFacts['itemsViewMode']
            evidence = $settingsEvidence
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.itemsUserSettingID'
            value = $tailFacts['itemsUserSettingID']
            evidence = $settingsEvidence
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.default-value'
            value = $defaultFact
            evidence = $settingsEvidence
        },
        [ordered]@{
            key = 'form.DynamicListExtInfo.listSettings.delegate'
            value = $listSettingsFact
            evidence = $formEvidence
        }
    )
    missingKeys = @(
        [ordered]@{
            key = 'dcs.settings.document.qname'
            status = 'not-proven-by-this-extractor'
            reason = 'The scoped extractor verifies only DcsV8Serializer.writeSettings and ListSettingsWriter.write; the settings method receives its physical QName from its caller.'
        },
        [ordered]@{
            key = 'form.DynamicListExtInfo.listSettings.qname'
            status = 'not-proven-by-this-extractor'
            reason = 'The scoped ListSettingsWriter method obtains the feature QName through IQNameProvider.getElementQName; this extractor does not inspect that provider implementation.'
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.type-id'
            status = 'not-proven-by-this-extractor'
            reason = 'The two scoped methods prove no complete caller wrapper or type-qualification contract.'
        },
        [ordered]@{
            key = 'dcs.DataCompositionSettings.opaque-extension.placement'
            status = 'not-proven-by-this-extractor'
            reason = 'The two scoped methods do not define placement for canonical opaque extensions.'
        }
    )
}

$jsonFirst = ConvertTo-DeterministicJson $report
$jsonSecond = ConvertTo-DeterministicJson $report
if ($jsonFirst -cne $jsonSecond) {
    throw 'DCS writer evidence report generation is nondeterministic.'
}
Write-Utf8LfFile $OutputReport $jsonFirst
Write-Output "Wrote $([System.IO.Path]::GetFullPath($OutputReport))"
Write-Output "verified=$($report.verifiedFacts.Count) missing=$($report.missingKeys.Count)"
