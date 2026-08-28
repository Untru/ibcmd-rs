<#
.SYNOPSIS
Extracts sanitized ChoiceParameterLinks writer evidence from EDT 2025.2.3+30.

.DESCRIPTION
Research-only tool. It reads an external EDT Convector inventory and sibling
installed EDT bundles, invokes javap, and retains only parser-proven facts plus
portable edt-derived:// coordinates. It never copies JAR/class/bytecode payload.

.EXAMPLE
pwsh tools/report-edt-form-choice-parameter-links-evidence.ps1 `
  -InputInventory 'C:\...\edt-models\inventory.json' `
  -EdtRelease 2025.2.3+30 `
  -OutputReport crates/ibcmd-schema/data/edt-2025.2.3-form-choice-parameter-links-writer-evidence.json

.EXAMPLE
pwsh tools/report-edt-form-choice-parameter-links-evidence.ps1 -SelfTest
#>
[CmdletBinding(DefaultParameterSetName = 'Extract')]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$InputInventory,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$OutputReport,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$EdtRelease,

    [Parameter(ParameterSetName = 'Extract')]
    [switch]$VerifyDeterminism,

    [Parameter(Mandatory = $true, ParameterSetName = 'SelfTest')]
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$acceptedRelease = '2025.2.3+30'
$bundleContracts = [ordered]@{
    'com._1c.g5.v8.dt.export.xml' = '13.0.100.v202602241426'
    'com._1c.g5.v8.dt.form.export.xml' = '10.1.0.v202602241426'
    'com._1c.g5.v8.dt.form.extension.export.xml' = '1.0.2900.v202602241426'
    'com._1c.g5.v8.dt.form.model' = '14.0.0.v202602241426'
    'com._1c.g5.v8.dt.metadata' = '18.0.100.v202602241426'
}

function Assert-ExactRelease {
    param([Parameter(Mandatory)] [string]$Release)
    if ($Release -cne $acceptedRelease) {
        throw "Only exact EDT release '$acceptedRelease' is accepted; found '$Release'."
    }
}

function Assert-BundleContract {
    param(
        [Parameter(Mandatory)] [string]$Name,
        [Parameter(Mandatory)] [string]$Version
    )
    if (-not $bundleContracts.Contains($Name)) {
        throw "Bundle '$Name' is outside the accepted evidence contract."
    }
    if ([string]$bundleContracts[$Name] -cne $Version) {
        throw "Bundle '$Name' version '$Version' differs from exact '$($bundleContracts[$Name])'."
    }
}

function Assert-TopLevelInventoryArray {
    param([Parameter(Mandatory)] [string]$Json)
    $trimmed = $Json.Trim()
    if ($trimmed.Length -lt 2 -or $trimmed[0] -cne '[' -or $trimmed[$trimmed.Length - 1] -cne ']') {
        throw 'External research inventory must be a top-level JSON array.'
    }
}

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
        if ($Lines[$index] -match '^  (?:public |private |protected ).+\(.*\).*;$' -or
            $Lines[$index] -match '^  static \{\};$') {
            $end = $index
            break
        }
    }
    return @($Lines[$start..($end - 1)])
}

function Assert-JavapMethodDescriptor {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$MethodBlock,
        [Parameter(Mandatory)] [string]$ExpectedDescriptor
    )
    $descriptors = @($MethodBlock | Where-Object {
        $_ -match '^\s+descriptor:\s+(.+)$'
    } | ForEach-Object {
        if ($_ -notmatch '^\s+descriptor:\s+(.+)$') {
            throw 'Internal descriptor parser disagreement.'
        }
        [string]$Matches[1]
    })
    if ($descriptors.Count -ne 1) {
        throw "Expected exactly one JVM descriptor; found $($descriptors.Count)."
    }
    if ($descriptors[0] -cne $ExpectedDescriptor) {
        throw "JVM descriptor '$($descriptors[0])' differs from exact '$ExpectedDescriptor'."
    }
}

function Get-VerifiedJavapMethodBlock {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] [string]$HeaderPattern,
        [Parameter(Mandatory)] [string]$Descriptor
    )
    $block = @(Get-JavapMethodBlock $Lines $HeaderPattern)
    Assert-JavapMethodDescriptor $block $Descriptor
    return $block
}

function Assert-ExactClassDeclaration {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] [string]$ExpectedDeclaration
    )
    $declarations = @($Lines | Where-Object { $_ -match '^public class ' })
    if ($declarations.Count -ne 1 -or [string]$declarations[0] -cne $ExpectedDeclaration) {
        throw "Class hierarchy differs from exact '$ExpectedDeclaration'."
    }
}

function Assert-ExactClassDeclarationAndDeclaredMethods {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] [string]$ExpectedDeclaration,
        [Parameter(Mandatory)] [string[]]$ExpectedMethodHeaders
    )
    Assert-ExactClassDeclaration $Lines $ExpectedDeclaration
    $headers = @($Lines | Where-Object {
        $_ -match '^  (?:public |private |protected ).+\(.*\).*;$'
    })
    if ([string]::Join('|', $headers) -cne [string]::Join('|', $ExpectedMethodHeaders)) {
        throw "Declared method set differs; a relevant QName override may be present: $([string]::Join('|', $headers))."
    }
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
        $pool[$index] = [ordered]@{ kind = $kind; target = $target; text = $line.Trim() }
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
    if ([string]$Instruction['opcode'] -cne $Opcode) {
        throw "Expected opcode '$Opcode', found '$($Instruction['text'])'."
    }
    if ($PSBoundParameters.ContainsKey('Operand') -and [string]$Instruction['operand'] -cne $Operand) {
        throw "Expected operand '$Operand', found '$($Instruction['text'])'."
    }
}

function Get-ConstantPoolIndex {
    param([Parameter(Mandatory)] $Instruction)
    if ([string]$Instruction['operand'] -notmatch '^#(\d+)(?:,.*)?$') {
        throw "Instruction has no constant-pool operand: '$($Instruction['text'])'."
    }
    return [int]$Matches[1]
}

function Assert-CpInstruction {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] $ConstantPool,
        [Parameter(Mandatory)] [int]$Offset,
        [Parameter(Mandatory)] [string]$Opcode,
        [Parameter(Mandatory)] [string]$Kind,
        [Parameter(Mandatory)] [string]$TargetPattern
    )
    $instruction = Get-InstructionAtOffset $Instructions $Offset
    Assert-Instruction $instruction $Opcode
    $cpIndex = Get-ConstantPoolIndex $instruction
    if (-not $ConstantPool.ContainsKey($cpIndex)) {
        throw "Instruction at $Offset refers to absent constant-pool entry #$cpIndex."
    }
    $entry = $ConstantPool[$cpIndex]
    if ([string]$entry['kind'] -cne $Kind -or [string]$entry['target'] -notmatch $TargetPattern) {
        throw "Unexpected constant-pool entry #${cpIndex}: '$($entry['text'])'."
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
        throw 'Internal extractor error: offset/opcode lengths differ.'
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
            throw "Expected offset $($Offsets[$index]), found '$($instruction['text'])'."
        }
        Assert-Instruction $instruction $Opcodes[$index]
    }
}

function Get-BranchGraph {
    param([Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions)
    return @($Instructions | Where-Object {
        [string]$_['opcode'] -match '^(?:if[a-z_]*|goto|tableswitch|lookupswitch|jsr)$'
    } | ForEach-Object { "$($_['offset']):$($_['opcode']):$($_['operand'])" })
}

function Assert-MethodEnvelope {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] [int]$Count,
        [Parameter(Mandatory)] [AllowEmptyCollection()] [string[]]$Branches,
        [Parameter(Mandatory)] [int]$LastOffset,
        [Parameter(Mandatory)] [string]$LastOpcode
    )
    if ($Instructions.Count -ne $Count) {
        throw "Instruction count $($Instructions.Count) differs from exact $Count."
    }
    if (@($Instructions | Where-Object { [string]$_['opcode'] -eq 'invokedynamic' }).Count -ne 0) {
        throw 'Scoped method unexpectedly contains invokedynamic.'
    }
    $actual = @(Get-BranchGraph $Instructions)
    if ([string]::Join('|', $actual) -cne [string]::Join('|', $Branches)) {
        throw "Unknown branch graph: $([string]::Join('|', $actual))."
    }
    $last = $Instructions[-1]
    if ([int]$last['offset'] -ne $LastOffset) {
        throw "Final instruction offset '$($last['offset'])' differs from '$LastOffset'."
    }
    Assert-Instruction $last $LastOpcode
}

function Assert-FeatureQNameMapping {
    param(
        [Parameter(Mandatory)] [object[]]$Instructions,
        [Parameter(Mandatory)] $Pool,
        [Parameter(Mandatory)] [string]$FeaturePattern,
        [Parameter(Mandatory)] [string]$QNamePattern
    )
    $foundIndexes = @()
    for ($index = 0; $index -lt $Instructions.Count - 3; $index++) {
        $instruction = $Instructions[$index]
        if ([string]$instruction['opcode'] -ne 'getstatic') { continue }
        $cpIndex = Get-ConstantPoolIndex $instruction
        if ($Pool.ContainsKey($cpIndex) -and [string]$Pool[$cpIndex]['target'] -match $FeaturePattern) {
            $foundIndexes += $index
        }
    }
    if ($foundIndexes.Count -ne 1) {
        throw "Expected one feature mapping '$FeaturePattern'; found $($foundIndexes.Count)."
    }
    $index = $foundIndexes[0]
    Assert-CpInstruction $Instructions $Pool ([int]$Instructions[$index + 1]['offset']) `
        'getstatic' 'Fieldref' $QNamePattern | Out-Null
    Assert-CpInstruction $Instructions $Pool ([int]$Instructions[$index + 2]['offset']) `
        'invokevirtual' 'Methodref' `
        'com/google/common/collect/ImmutableMap\$Builder\.put:\(Ljava/lang/Object;Ljava/lang/Object;\)Lcom/google/common/collect/ImmutableMap\$Builder;$' | Out-Null
    Assert-Instruction $Instructions[$index + 3] 'pop'
}

function Assert-ExactFeatureNameMapEnvelope {
    param(
        [Parameter(Mandatory)] [object[]]$Instructions,
        [Parameter(Mandatory)] $Pool
    )
    Assert-MethodEnvelope $Instructions 286 @() 627 'return'
    for ($index = 0; $index -lt 285; $index += 5) {
        $group = @($Instructions[$index..($index + 4)])
        $expected = @('aload_1', 'getstatic', 'getstatic', 'invokevirtual', 'pop')
        for ($n = 0; $n -lt $expected.Count; $n++) {
            Assert-Instruction $group[$n] $expected[$n]
        }
        Assert-CpInstruction $group $Pool ([int]$group[1]['offset']) 'getstatic' 'Fieldref' `
            '.+\$Literals\.[A-Z0-9_]+:Lorg/eclipse/emf/ecore/(?:EAttribute|EReference);$' | Out-Null
        Assert-CpInstruction $group $Pool ([int]$group[2]['offset']) 'getstatic' 'Fieldref' `
            '.+\.[A-Z0-9_]+:Ljavax/xml/namespace/QName;$' | Out-Null
        Assert-CpInstruction $group $Pool ([int]$group[3]['offset']) 'invokevirtual' 'Methodref' `
            'com/google/common/collect/ImmutableMap\$Builder\.put:\(Ljava/lang/Object;Ljava/lang/Object;\)Lcom/google/common/collect/ImmutableMap\$Builder;$' | Out-Null
    }
}

function Get-QNameFromStaticInitializer {
    param(
        [Parameter(Mandatory)] [object[]]$Instructions,
        [Parameter(Mandatory)] $Pool,
        [Parameter(Mandatory)] [string]$ClassPattern,
        [Parameter(Mandatory)] [string]$Field,
        [Parameter(Mandatory)] [string]$ExpectedNamespace,
        [Parameter(Mandatory)] [string]$ExpectedLocal,
        [Parameter(Mandatory)] [string]$ExpectedPrefix
    )
    $fieldPattern = "${ClassPattern}\.${Field}:Ljavax/xml/namespace/QName;"
    $foundIndexes = @()
    for ($index = 0; $index -lt $Instructions.Count; $index++) {
        if ([string]$Instructions[$index]['opcode'] -ne 'putstatic') { continue }
        $cpIndex = Get-ConstantPoolIndex $Instructions[$index]
        if ($Pool.ContainsKey($cpIndex) -and [string]$Pool[$cpIndex]['target'] -match $fieldPattern) {
            $foundIndexes += $index
        }
    }
    if ($foundIndexes.Count -ne 1) {
        throw "Expected exactly one QName initializer for '$Field'; found $($foundIndexes.Count)."
    }
    $index = $foundIndexes[0]
    if ($index -lt 6) { throw "QName initializer for '$Field' is truncated." }
    $slice = @($Instructions[($index - 6)..$index])
    $expected = @('new', 'dup', 'ldc', 'ldc', 'ldc', 'invokespecial', 'putstatic')
    for ($n = 0; $n -lt $expected.Count; $n++) {
        Assert-Instruction $slice[$n] $expected[$n]
    }
    Assert-CpInstruction $slice $Pool ([int]$slice[0]['offset']) 'new' 'Class' 'javax/xml/namespace/QName$' | Out-Null
    $namespace = Assert-CpInstruction $slice $Pool ([int]$slice[2]['offset']) 'ldc' 'String' '.*'
    $local = Assert-CpInstruction $slice $Pool ([int]$slice[3]['offset']) 'ldc' 'String' '.*'
    $prefix = Assert-CpInstruction $slice $Pool ([int]$slice[4]['offset']) 'ldc' 'String' '.*'
    Assert-CpInstruction $slice $Pool ([int]$slice[5]['offset']) 'invokespecial' 'Methodref' `
        'javax/xml/namespace/QName\."<init>":\(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;\)V$' | Out-Null
    if ($namespace -cne $ExpectedNamespace -or $local -cne $ExpectedLocal -or $prefix -cne $ExpectedPrefix) {
        throw "QName '$Field' is '{$namespace}$local' prefix '$prefix', not accepted value."
    }
    return [ordered]@{ qname = "{$namespace}$local"; prefix = $prefix }
}

function Get-OwnerWriterFact {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 83 @(
        '9:if_acmpeq:33', '34:ifnonnull:38', '76:ifeq:169', '93:ifne:155',
        '111:goto:138', '145:ifne:114', '152:goto:189', '157:ifeq:189',
        '166:goto:189', '171:ifnull:189'
    ) 189 'return'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 34) 'ifnonnull' '38'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 93) 'ifne' '155'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 157) 'ifeq' '189'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 171) 'ifnull' '189'
    Assert-CpInstruction $Instructions $Pool 6 'getstatic' 'Fieldref' `
        'FormPackage\$Literals\.FORM_CHOICE_PARAMETER_LINK:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 43 'invokeinterface' 'InterfaceMethodref' `
        'IQNameProvider\.getElementQName:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 71 'invokeinterface' 'InterfaceMethodref' `
        'EStructuralFeature\.isMany:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 88 'invokeinterface' 'InterfaceMethodref' `
        'java/util/List\.isEmpty:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 99 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeStartElement:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 135 'invokevirtual' 'Methodref' `
        'FormChoiceParameterLinkWriter\.writeFormChoiceParameterLink:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 149 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeEndElement:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 163 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeEmptyElement:' | Out-Null
    return [ordered]@{
        emptyCollection = [ordered]@{
            writeEmptyArgumentTrue = 'write-empty-owner-wrapper'
            writeEmptyArgumentFalse = 'omit-owner-wrapper'
        }
        null = [ordered]@{
            ownerEObject = 'omit'
            singularFeatureValue = 'omit'
            manyFeatureValue = 'not-accepted-no-null-branch-before-List.isEmpty'
        }
    }
}

function Get-ItemWriterFact {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 49 @('34:ifnull:78', '75:goto:85') 106 'return'
    $targets = [ordered]@{
        1 = 'IXmlElements\$XR\.LINK:'
        13 = 'CommonPackage\$Literals\.ABSTRACT_CHOICE_PARAMETER_LINK__NAME:'
        25 = 'FormChoiceParameterLink\.getDatapath:'
        43 = 'FormChoiceParameterLinkWriter\.getDataPathSegments:'
        49 = 'IXmlElements\$XR\.DATA_PATH:'
        56 = 'IXmlElements\$XSI\.TYPE:'
        59 = 'IXmlElements\$XS\.STRING:'
        79 = 'IXmlElements\$XR\.DATA_PATH:'
        91 = 'CommonPackage\$Literals\.ABSTRACT_CHOICE_PARAMETER_LINK__CHANGE_MODE:'
    }
    foreach ($targetEntry in $targets.GetEnumerator()) {
        $offset = [int]$targetEntry.Key
        $targetPattern = [string]$targetEntry.Value
        $opcode = if ($offset -in @(25)) { 'invokeinterface' } elseif ($offset -eq 43) {
            'invokevirtual'
        } else { 'getstatic' }
        $kind = if ($opcode -eq 'invokeinterface') { 'InterfaceMethodref' } elseif ($opcode -eq 'invokevirtual') {
            'Methodref'
        } else { 'Fieldref' }
        Assert-CpInstruction $Instructions $Pool $offset $opcode $kind $targetPattern | Out-Null
    }
    Assert-Instruction (Get-InstructionAtOffset $Instructions 16) 'iconst_0'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 34) 'ifnull' '78'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 75) 'goto' '85'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 94) 'iconst_1'
    Assert-CpInstruction $Instructions $Pool 62 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeAttribute:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 68 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeCharacters:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 72 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeInlineEndElement:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 82 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeEmptyElement:' | Out-Null
    return [ordered]@{
        order = @('name', 'datapath', 'changeMode')
        nameFeatureWriterBoolean = $false
        changeModeFeatureWriterBoolean = $true
        datapathNull = 'write-empty-DataPath-element'
    }
}

function Get-RegularDataPathFact {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 6 @() 14 'areturn'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 0) 'bipush' '46'
    Assert-CpInstruction $Instructions $Pool 2 'invokestatic' 'Methodref' `
        'com/google/common/base/Joiner\.on:\(C\)' | Out-Null
    Assert-CpInstruction $Instructions $Pool 6 'invokeinterface' 'InterfaceMethodref' `
        'AbstractDataPath\.getSegments:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 11 'invokevirtual' 'Methodref' `
        'com/google/common/base/Joiner\.join:' | Out-Null
    return [ordered]@{
        delegate = 'FormChoiceParameterLinkWriter.getDataPathSegments'
        segmentSource = 'AbstractDataPath.getSegments'
        delimiter = '.'
    }
}

function Assert-ExtensionFallbackInstruction {
    param([object[]]$Instructions, $Pool)
    Assert-CpInstruction $Instructions $Pool 73 'invokespecial' 'Methodref' `
        'FormChoiceParameterLinkWriter\.getDataPathSegments:' | Out-Null
}

function Get-ExtensionFact {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 33 @(
        '6:ifnull:32', '20:ifne:32', '29:goto:33', '46:ifeq:69', '63:ifeq:69'
    ) 76 'areturn'
    Assert-CpInstruction $Instructions $Pool 1 'invokeinterface' 'InterfaceMethodref' `
        'Form\.getExtensionForm:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 15 'invokeinterface' 'InterfaceMethodref' `
        'Form\.eIsProxy:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 41 'invokeinterface' 'InterfaceMethodref' `
        'IFormExtensionService\.isExtensionAdopted:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 58 'invokeinterface' 'InterfaceMethodref' `
        'IFormExtensionService\.shouldSkipForExport:' | Out-Null
    Assert-Instruction (Get-InstructionAtOffset $Instructions 56) 'aconst_null'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 57) 'aconst_null'
    $sentinel = Assert-CpInstruction $Instructions $Pool 66 'ldc' 'String' '^0$'
    Assert-ExtensionFallbackInstruction $Instructions $Pool
    return [ordered]@{
        selectedForm = 'extensionForm when non-null and not proxy; otherwise original form'
        skipPredicate = 'isExtensionAdopted(selectedForm) && shouldSkipForExport(selectedForm, datapath, null, null)'
        skipResult = $sentinel
        otherwise = 'invoke-super FormChoiceParameterLinkWriter.getDataPathSegments'
    }
}

function Get-OrderFact {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 341 @(
        '195:ifeq:310', '533:ifeq:618', '599:ifeq:610', '641:iflt:684'
    ) 688 'areturn'
    Assert-ExactSubsequence $Instructions 422 `
        @(422, 423, 426, 429, 430, 431, 434, 437, 438, 439, 442, 445) `
        @('aload_2', 'getstatic', 'invokevirtual', 'pop', 'aload_2', 'getstatic',
          'invokevirtual', 'pop', 'aload_2', 'getstatic', 'invokevirtual', 'pop')
    Assert-CpInstruction $Instructions $Pool 423 'getstatic' 'Fieldref' `
        'INPUT_FIELD_EXT_INFO__CHOICE_FORM:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 431 'getstatic' 'Fieldref' `
        'INPUT_FIELD_EXT_INFO__CHOICE_PARAMETER_LINKS:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 439 'getstatic' 'Fieldref' `
        'INPUT_FIELD_EXT_INFO__CHOICE_PARAMETERS:' | Out-Null
    return [ordered]@{
        predecessor = 'choiceForm'
        feature = 'choiceParameterLinks'
        successor = 'choiceParameters'
        version = 'unconditional-in-getInputFieldExtInfoFeatures'
    }
}

function Get-CommonChoiceParameterLinkDefaultFacts {
    param([object[]]$Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 3040 @('4:ifeq:8') 6111 'return'
    Assert-ExactSubsequence $Instructions 222 `
        @(222,223,224,227,228,233,236,237,238,239,242,243,244,245,246,247,248,
          249,250,253,254,255,256,259,260,263,266,267,268,269,272,273,274,275,
          276,277,278,279,280,283) `
        @('aload_0','aload_0','invokevirtual','aload_1','invokeinterface','ldc_w',
          'aconst_null','iconst_1','iconst_1','ldc_w','iconst_0','iconst_0',
          'iconst_1','iconst_0','iconst_0','iconst_0','iconst_0','iconst_1',
          'invokevirtual','pop','aload_0','aload_0','invokevirtual','aload_0',
          'invokevirtual','ldc_w','aconst_null','iconst_0','iconst_1','ldc_w',
          'iconst_0','iconst_0','iconst_1','iconst_0','iconst_0','iconst_0',
          'iconst_0','iconst_1','invokevirtual','pop')
    Assert-CpInstruction $Instructions $Pool 224 'invokevirtual' 'Methodref' `
        'getAbstractChoiceParameterLink_Name:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 233 'ldc_w' 'String' '^name$' | Out-Null
    Assert-CpInstruction $Instructions $Pool 250 'invokevirtual' 'Methodref' `
        'initEAttribute:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 256 'invokevirtual' 'Methodref' `
        'getAbstractChoiceParameterLink_ChangeMode:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 260 'invokevirtual' 'Methodref' `
        'getLinkedValueChangeMode:' | Out-Null
    Assert-CpInstruction $Instructions $Pool 263 'ldc_w' 'String' '^changeMode$' | Out-Null
    Assert-CpInstruction $Instructions $Pool 280 'invokevirtual' 'Methodref' `
        'initEAttribute:' | Out-Null
    return [ordered]@{
        name = [ordered]@{ modelDefault = $null; lowerBound = 1; upperBound = 1 }
        changeMode = [ordered]@{ modelDefault = $null; lowerBound = 0; upperBound = 1 }
    }
}

function Get-ModelFacts {
    param(
        [object[]]$FormInstructions, $FormPool,
        [object[]]$CommonInstructions, $CommonPool
    )
    Assert-MethodEnvelope $FormInstructions 27386 @('4:ifeq:8') 48186 'return'
    Assert-ExactSubsequence $FormInstructions 19656 `
        @(19656,19657,19658,19661,19662,19665,19666,19669,19670,19671,19672,
          19675,19676,19677,19678,19679,19680,19681,19682,19683,19684,19687) `
        @('aload_0','aload_0','invokevirtual','aload_0','invokevirtual','aconst_null',
          'ldc_w','aconst_null','iconst_0','iconst_m1','ldc_w','iconst_0','iconst_0',
          'iconst_1','iconst_1','iconst_0','iconst_0','iconst_1','iconst_0','iconst_1',
          'invokevirtual','pop')
    Assert-CpInstruction $FormInstructions $FormPool 19658 'invokevirtual' 'Methodref' `
        'getInputFieldExtInfo_ChoiceParameterLinks:' | Out-Null
    Assert-CpInstruction $FormInstructions $FormPool 19662 'invokevirtual' 'Methodref' `
        'getFormChoiceParameterLink:' | Out-Null
    $ownerName = Assert-CpInstruction $FormInstructions $FormPool 19666 'ldc_w' 'String' `
        '^choiceParameterLinks$'
    Assert-CpInstruction $FormInstructions $FormPool 19684 'invokevirtual' 'Methodref' `
        'initEReference:' | Out-Null

    $defaults = Get-CommonChoiceParameterLinkDefaultFacts $CommonInstructions $CommonPool
    return [ordered]@{
        ownerFeatureName = $ownerName
        ownerModelDefault = $null
        ownerLowerBound = 0
        ownerUpperBound = 'unbounded'
        name = $defaults.name
        changeMode = $defaults.changeMode
    }
}

function Get-LexicalMap {
    param([object[]]$StaticInstructions, $Pool, [object[]]$ToStringInstructions)
    Assert-MethodEnvelope $StaticInstructions 45 @() 86 'return'
    Assert-ExactSubsequence $StaticInstructions 0 `
        @(0,3,4,6,7,8,10,12,15,18,21,22,24,25,26,28,30,33) `
        @('new','dup','ldc','iconst_0','iconst_0','ldc','ldc','invokespecial',
          'putstatic','new','dup','ldc','iconst_1','iconst_1','ldc','ldc',
          'invokespecial','putstatic')
    Assert-CpInstruction $StaticInstructions $Pool 4 'ldc' 'String' '^CLEAR$' | Out-Null
    $clearName = Assert-CpInstruction $StaticInstructions $Pool 8 'ldc' 'String' '^Clear$'
    $clearLiteral = Assert-CpInstruction $StaticInstructions $Pool 10 'ldc' 'String' '^Clear$'
    Assert-CpInstruction $StaticInstructions $Pool 15 'putstatic' 'Fieldref' `
        'LinkedValueChangeMode\.CLEAR:' | Out-Null
    Assert-CpInstruction $StaticInstructions $Pool 22 'ldc' 'String' '^DONT_CHANGE$' | Out-Null
    $dontName = Assert-CpInstruction $StaticInstructions $Pool 26 'ldc' 'String' '^DontChange$'
    $dontLiteral = Assert-CpInstruction $StaticInstructions $Pool 28 'ldc' 'String' '^DontChange$'
    Assert-CpInstruction $StaticInstructions $Pool 33 'putstatic' 'Fieldref' `
        'LinkedValueChangeMode\.DONT_CHANGE:' | Out-Null
    Assert-MethodEnvelope $ToStringInstructions 3 @() 4 'areturn'
    Assert-CpInstruction $ToStringInstructions $Pool 1 'getfield' 'Fieldref' `
        'LinkedValueChangeMode\.literal:' | Out-Null
    if ($clearName -cne $clearLiteral -or $dontName -cne $dontLiteral) {
        throw 'LinkedValueChangeMode name/literal disagreement.'
    }
    return [ordered]@{
        CLEAR = $clearLiteral
        DONT_CHANGE = $dontLiteral
    }
}

function Get-ReadOnlyManifest {
    param([Parameter(Mandatory)] [string]$Jar)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($Jar)
    try {
        $entry = $archive.GetEntry('META-INF/MANIFEST.MF')
        if ($null -eq $entry) { throw "JAR '$Jar' has no manifest." }
        $reader = [System.IO.StreamReader]::new($entry.Open(), [System.Text.Encoding]::UTF8)
        try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
    }
    finally {
        $archive.Dispose()
    }
    $unfolded = $text -replace "\r?\n ", ''
    $values = @{}
    foreach ($line in ($unfolded -split "\r?\n")) {
        if ($line -match '^([^:]+):\s*(.*)$') {
            $values[[string]$Matches[1]] = [string]$Matches[2]
        }
    }
    return $values
}

function Resolve-ResearchBundle {
    param(
        [Parameter(Mandatory)] [object[]]$Inventory,
        [Parameter(Mandatory)] [string[]]$PluginDirectories,
        [Parameter(Mandatory)] [string]$Name
    )
    $version = [string]$bundleContracts[$Name]
    Assert-BundleContract $Name $version
    $entries = @($Inventory | Where-Object { [string]$_.bundle -ceq $Name })
    if ($entries.Count -gt 1) {
        throw "Inventory contains ambiguous '$Name' entries."
    }
    if ($entries.Count -eq 1) {
        $jar = [string]$entries[0].jar
    }
    else {
        $expectedFile = "${Name}_${version}.jar"
        $matches = @($PluginDirectories | ForEach-Object {
            $candidate = Join-Path $_ $expectedFile
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { $candidate }
        } | Sort-Object -Unique)
        if ($matches.Count -ne 1) {
            throw "Sibling installed EDT lookup for '$Name' found $($matches.Count) exact JARs."
        }
        $jar = $matches[0]
    }
    if (-not (Test-Path -LiteralPath $jar -PathType Leaf)) {
        throw "Bundle '$Name' does not provide an existing JAR."
    }
    $expectedLeaf = "${Name}_${version}.jar"
    if ([System.IO.Path]::GetFileName($jar) -cne $expectedLeaf) {
        throw "Bundle '$Name' JAR filename is not exact '$expectedLeaf'."
    }
    if ($jar -notmatch [regex]::Escape("1c-edt-$acceptedRelease-x86_64")) {
        throw "Bundle '$Name' is not under exact EDT '$acceptedRelease' installation."
    }
    $manifest = Get-ReadOnlyManifest $jar
    if (-not $manifest.ContainsKey('Bundle-SymbolicName') -or
        -not $manifest.ContainsKey('Bundle-Version')) {
        throw "Bundle '$Name' manifest lacks symbolic name/version."
    }
    $symbolicName = ([string]$manifest['Bundle-SymbolicName'] -split ';')[0]
    Assert-BundleContract $symbolicName ([string]$manifest['Bundle-Version'])
    if ($symbolicName -cne $Name) {
        throw "Resolved JAR for '$Name' identifies as '$symbolicName'."
    }
    return [ordered]@{ name = $Name; version = $version; jar = $jar }
}

function New-Evidence {
    param([string[]]$Sources, [string]$Note)
    return [ordered]@{
        kind = 'javap-v-exact-class-hierarchy-method-descriptor-control-flow-constant-pool'
        status = 'verified'
        sources = @($Sources)
        note = $Note
    }
}

function ConvertTo-DeterministicJson {
    param([Parameter(Mandatory)]$Value)
    return (($Value | ConvertTo-Json -Depth 20 -Compress).Replace("`r`n", "`n") + "`n")
}

function Write-Utf8LfFile {
    param([string]$Path, [string]$Text)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($fullPath)) | Out-Null
    [System.IO.File]::WriteAllText($fullPath, $Text, [System.Text.UTF8Encoding]::new($false))
}

function New-TestInstruction {
    param([int]$Offset, [string]$Opcode, [string]$Operand = '')
    return [ordered]@{ offset=$Offset; opcode=$Opcode; operand=$Operand; comment=''; text="$Offset`: $Opcode $Operand" }
}

function New-TestCpEntry {
    param([string]$Kind, [string]$Target)
    return [ordered]@{ kind=$Kind; target=$Target; text="$Kind // $Target" }
}

function New-SyntheticMethodInstructions {
    param(
        [Parameter(Mandatory)] [object[]]$Required,
        [Parameter(Mandatory)] [int]$Count
    )
    $offsets = @{}
    foreach ($instruction in $Required) {
        $offset = [int]$instruction['offset']
        if ($offsets.ContainsKey($offset)) {
            throw "Synthetic fixture has duplicate required offset $offset."
        }
        $offsets[$offset] = $true
    }
    $all = [System.Collections.Generic.List[object]]::new()
    foreach ($instruction in $Required) { $all.Add($instruction) }
    $fillerOffset = -1
    while ($all.Count -lt $Count) {
        while ($offsets.ContainsKey($fillerOffset)) { $fillerOffset-- }
        $all.Add((New-TestInstruction $fillerOffset 'nop'))
        $offsets[$fillerOffset] = $true
        $fillerOffset--
    }
    if ($all.Count -ne $Count) {
        throw "Synthetic fixture requires $($all.Count) instructions, exceeding exact $Count."
    }
    return @($all | Sort-Object { [int]$_['offset'] })
}

function Assert-SelfTestRejected {
    param([scriptblock]$Action, [string]$Name)
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw "Self-test '$Name' was not rejected." }
}

function Invoke-SelfTest {
    Assert-ExactRelease $acceptedRelease
    Assert-BundleContract 'com._1c.g5.v8.dt.form.export.xml' '10.1.0.v202602241426'
    Assert-TopLevelInventoryArray '[{"bundle":"synthetic"}]'
    Assert-SelfTestRejected { Assert-ExactRelease '2025.2.3+29' } 'wrong-release'
    Assert-SelfTestRejected {
        Assert-BundleContract 'com._1c.g5.v8.dt.form.export.xml' '10.1.1.v202602241426'
    } 'wrong-bundle'
    Assert-SelfTestRejected {
        Assert-TopLevelInventoryArray '{"bundle":"synthetic"}'
    } 'top-level-inventory-object'

    $methodFixture = @(
        '  public void write();',
        '    descriptor: ()V',
        '       0: return'
    )
    $null = Get-VerifiedJavapMethodBlock $methodFixture '^  public void write\(\);$' '()V'
    Assert-SelfTestRejected {
        Get-VerifiedJavapMethodBlock $methodFixture '^  public void write\(\);$' '(I)V'
    } 'wrong-method-descriptor'
    Assert-SelfTestRejected {
        Get-JavapMethodBlock @($methodFixture + $methodFixture) '^  public void write\(\);$'
    } 'ambiguous-method'
    Assert-SelfTestRejected {
        Get-JavapMethodBlock $methodFixture '^  private void missing\(\);$'
    } 'missing-method'

    $branchFixture = @(
        New-TestInstruction 0 'aload_0'
        New-TestInstruction 1 'ifnull' '5'
        New-TestInstruction 4 'return'
        New-TestInstruction 5 'return'
    )
    Assert-MethodEnvelope $branchFixture 4 @('1:ifnull:5') 5 'return'
    $badBranch = @($branchFixture | ForEach-Object {
        New-TestInstruction ([int]$_['offset']) ([string]$_['opcode']) ([string]$_['operand'])
    })
    $badBranch[1]['operand'] = '4'
    Assert-SelfTestRejected {
        Assert-MethodEnvelope $badBranch 4 @('1:ifnull:5') 5 'return'
    } 'control-flow'

    $ownerPool = @{
        1=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/form/model/FormPackage$Literals.FORM_CHOICE_PARAMETER_LINK:Lorg/eclipse/emf/ecore/EClass;'
        2=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/export/xml/IQNameProvider.getElementQName:(Lorg/eclipse/emf/ecore/EStructuralFeature;)Ljavax/xml/namespace/QName;'
        3=New-TestCpEntry 'InterfaceMethodref' 'org/eclipse/emf/ecore/EStructuralFeature.isMany:()Z'
        4=New-TestCpEntry 'InterfaceMethodref' 'java/util/List.isEmpty:()Z'
        5=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeStartElement:(Ljavax/xml/namespace/QName;)V'
        6=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/form/export/xml/writer/FormChoiceParameterLinkWriter.writeFormChoiceParameterLink:(Lx;)V'
        7=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeEndElement:()V'
        8=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeEmptyElement:(Ljavax/xml/namespace/QName;)V'
    }
    $ownerRequired = @(
        New-TestInstruction 6 'getstatic' '#1'
        New-TestInstruction 9 'if_acmpeq' '33'
        New-TestInstruction 34 'ifnonnull' '38'
        New-TestInstruction 43 'invokeinterface' '#2,  2'
        New-TestInstruction 71 'invokeinterface' '#3,  1'
        New-TestInstruction 76 'ifeq' '169'
        New-TestInstruction 88 'invokeinterface' '#4,  1'
        New-TestInstruction 93 'ifne' '155'
        New-TestInstruction 99 'invokevirtual' '#5'
        New-TestInstruction 111 'goto' '138'
        New-TestInstruction 135 'invokevirtual' '#6'
        New-TestInstruction 145 'ifne' '114'
        New-TestInstruction 149 'invokevirtual' '#7'
        New-TestInstruction 152 'goto' '189'
        New-TestInstruction 157 'ifeq' '189'
        New-TestInstruction 163 'invokevirtual' '#8'
        New-TestInstruction 166 'goto' '189'
        New-TestInstruction 171 'ifnull' '189'
        New-TestInstruction 189 'return'
    )
    $ownerFixture = @(New-SyntheticMethodInstructions $ownerRequired 83)
    $null = Get-OwnerWriterFact $ownerFixture $ownerPool
    $ownerAmbiguous = @($ownerFixture | Where-Object { [int]$_['offset'] -ne -1 })
    $ownerAmbiguous += New-TestInstruction 43 'invokeinterface' '#2,  2'
    $ownerAmbiguous = @($ownerAmbiguous | Sort-Object { [int]$_['offset'] })
    Assert-SelfTestRejected {
        Get-OwnerWriterFact $ownerAmbiguous $ownerPool
    } 'ambiguous-owner-qname-call'

    $itemPool = @{
        1=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/export/xml/IXmlElements$XR.LINK:Ljavax/xml/namespace/QName;'
        2=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/metadata/common/CommonPackage$Literals.ABSTRACT_CHOICE_PARAMETER_LINK__NAME:Lorg/eclipse/emf/ecore/EAttribute;'
        3=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/model/FormChoiceParameterLink.getDatapath:()Lx;'
        4=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/form/export/xml/writer/FormChoiceParameterLinkWriter.getDataPathSegments:(Lx;)Ljava/lang/String;'
        5=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/export/xml/IXmlElements$XR.DATA_PATH:Ljavax/xml/namespace/QName;'
        6=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/export/xml/IXmlElements$XSI.TYPE:Ljavax/xml/namespace/QName;'
        7=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/export/xml/IXmlElements$XS.STRING:Ljavax/xml/namespace/QName;'
        8=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/metadata/common/CommonPackage$Literals.ABSTRACT_CHOICE_PARAMETER_LINK__CHANGE_MODE:Lorg/eclipse/emf/ecore/EAttribute;'
        9=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeAttribute:(Lx;)V'
        10=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeCharacters:(Ljava/lang/String;)V'
        11=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeInlineEndElement:()V'
        12=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeEmptyElement:(Lx;)V'
    }
    $itemRequired = @(
        New-TestInstruction 1 'getstatic' '#1'
        New-TestInstruction 13 'getstatic' '#2'
        New-TestInstruction 16 'iconst_0'
        New-TestInstruction 25 'invokeinterface' '#3,  1'
        New-TestInstruction 34 'ifnull' '78'
        New-TestInstruction 43 'invokevirtual' '#4'
        New-TestInstruction 49 'getstatic' '#5'
        New-TestInstruction 56 'getstatic' '#6'
        New-TestInstruction 59 'getstatic' '#7'
        New-TestInstruction 62 'invokevirtual' '#9'
        New-TestInstruction 68 'invokevirtual' '#10'
        New-TestInstruction 72 'invokevirtual' '#11'
        New-TestInstruction 75 'goto' '85'
        New-TestInstruction 79 'getstatic' '#5'
        New-TestInstruction 82 'invokevirtual' '#12'
        New-TestInstruction 91 'getstatic' '#8'
        New-TestInstruction 94 'iconst_1'
        New-TestInstruction 106 'return'
    )
    $itemFixture = @(New-SyntheticMethodInstructions $itemRequired 49)
    $null = Get-ItemWriterFact $itemFixture $itemPool
    $badItemPool = $itemPool.Clone()
    $badItemPool[2] = $itemPool[8]
    $badItemPool[8] = $itemPool[2]
    Assert-SelfTestRejected {
        Get-ItemWriterFact $itemFixture $badItemPool
    } 'wrong-item-field-order'

    $orderPool = @{
        1=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/form/model/FormPackage$Literals.INPUT_FIELD_EXT_INFO__CHOICE_FORM:Lorg/eclipse/emf/ecore/EReference;'
        2=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/form/model/FormPackage$Literals.INPUT_FIELD_EXT_INFO__CHOICE_PARAMETER_LINKS:Lorg/eclipse/emf/ecore/EReference;'
        3=New-TestCpEntry 'Fieldref' 'com/_1c/g5/v8/dt/form/model/FormPackage$Literals.INPUT_FIELD_EXT_INFO__CHOICE_PARAMETERS:Lorg/eclipse/emf/ecore/EReference;'
    }
    $orderRequired = @(
        New-TestInstruction 195 'ifeq' '310'
        New-TestInstruction 422 'aload_2'
        New-TestInstruction 423 'getstatic' '#1'
        New-TestInstruction 426 'invokevirtual' '#10'
        New-TestInstruction 429 'pop'
        New-TestInstruction 430 'aload_2'
        New-TestInstruction 431 'getstatic' '#2'
        New-TestInstruction 434 'invokevirtual' '#10'
        New-TestInstruction 437 'pop'
        New-TestInstruction 438 'aload_2'
        New-TestInstruction 439 'getstatic' '#3'
        New-TestInstruction 442 'invokevirtual' '#10'
        New-TestInstruction 445 'pop'
        New-TestInstruction 533 'ifeq' '618'
        New-TestInstruction 599 'ifeq' '610'
        New-TestInstruction 641 'iflt' '684'
        New-TestInstruction 688 'areturn'
    )
    $orderFixture = @(New-SyntheticMethodInstructions $orderRequired 341)
    $null = Get-OrderFact $orderFixture $orderPool
    $badOrderPool = $orderPool.Clone()
    $badOrderPool[1] = $orderPool[3]
    Assert-SelfTestRejected {
        Get-OrderFact $orderFixture $badOrderPool
    } 'wrong-owner-feature-order'

    $extensionPool = @{
        1=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/model/Form.getExtensionForm:()Lx;'
        2=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/model/Form.eIsProxy:()Z'
        3=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/service/extension/IFormExtensionService.isExtensionAdopted:(Ljava/lang/Object;)Z'
        4=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/service/extension/IFormExtensionService.shouldSkipForExport:(Lx;)Z'
        5=New-TestCpEntry 'String' '0'
        6=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/form/export/xml/writer/FormChoiceParameterLinkWriter.getDataPathSegments:(Lx;)Ljava/lang/String;'
    }
    $extensionRequired = @(
        New-TestInstruction 1 'invokeinterface' '#1,  1'
        New-TestInstruction 6 'ifnull' '32'
        New-TestInstruction 15 'invokeinterface' '#2,  1'
        New-TestInstruction 20 'ifne' '32'
        New-TestInstruction 29 'goto' '33'
        New-TestInstruction 41 'invokeinterface' '#3,  2'
        New-TestInstruction 46 'ifeq' '69'
        New-TestInstruction 56 'aconst_null'
        New-TestInstruction 57 'aconst_null'
        New-TestInstruction 58 'invokeinterface' '#4,  5'
        New-TestInstruction 63 'ifeq' '69'
        New-TestInstruction 66 'ldc' '#5'
        New-TestInstruction 73 'invokespecial' '#6'
        New-TestInstruction 76 'areturn'
    )
    $extensionFixture = @(New-SyntheticMethodInstructions $extensionRequired 33)
    $null = Get-ExtensionFact $extensionFixture $extensionPool
    $extensionMissing = @($extensionFixture | ForEach-Object {
        if ([int]$_['offset'] -eq 58) { New-TestInstruction 58 'nop' } else { $_ }
    })
    Assert-SelfTestRejected {
        Get-ExtensionFact $extensionMissing $extensionPool
    } 'missing-extension-skip-call'

    $qPool = @{
        1=New-TestCpEntry 'Class' 'javax/xml/namespace/QName'
        2=New-TestCpEntry 'String' 'urn:test'
        3=New-TestCpEntry 'String' 'Link'
        4=New-TestCpEntry 'String' 'xr'
        5=New-TestCpEntry 'Methodref' 'javax/xml/namespace/QName."<init>":(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V'
        6=New-TestCpEntry 'Fieldref' 'com/example/IXmlElements$XR.LINK:Ljavax/xml/namespace/QName;'
    }
    $qInstructions = @(
        New-TestInstruction 0 'new' '#1'
        New-TestInstruction 3 'dup'
        New-TestInstruction 4 'ldc' '#2'
        New-TestInstruction 6 'ldc' '#3'
        New-TestInstruction 8 'ldc' '#4'
        New-TestInstruction 10 'invokespecial' '#5'
        New-TestInstruction 13 'putstatic' '#6'
    )
    $null = Get-QNameFromStaticInitializer $qInstructions $qPool `
        'com/example/IXmlElements\$XR' 'LINK' 'urn:test' 'Link' 'xr'
    $badQNamePool = $qPool.Clone()
    $badQNamePool[3] = New-TestCpEntry 'String' 'Other'
    Assert-SelfTestRejected {
        Get-QNameFromStaticInitializer $qInstructions $badQNamePool `
            'com/example/IXmlElements\$XR' 'LINK' 'urn:test' 'Link' 'xr'
    } 'qname'

    $regularPool = @{
        1=New-TestCpEntry 'Methodref' 'com/google/common/base/Joiner.on:(C)Lx;'
        2=New-TestCpEntry 'InterfaceMethodref' 'com/_1c/g5/v8/dt/form/model/AbstractDataPath.getSegments:()Lx;'
        3=New-TestCpEntry 'Methodref' 'com/google/common/base/Joiner.join:(Ljava/lang/Iterable;)Ljava/lang/String;'
    }
    $regularInstructions = @(
        New-TestInstruction 0 'bipush' '46'
        New-TestInstruction 2 'invokestatic' '#1'
        New-TestInstruction 5 'aload_2'
        New-TestInstruction 6 'invokeinterface' '#2,  1'
        New-TestInstruction 11 'invokevirtual' '#3'
        New-TestInstruction 14 'areturn'
    )
    $null = Get-RegularDataPathFact $regularInstructions $regularPool
    $badDelegatePool = $regularPool.Clone()
    $badDelegatePool[3] = New-TestCpEntry 'Methodref' 'com/example/Other.join:(Ljava/lang/Iterable;)Ljava/lang/String;'
    Assert-SelfTestRejected {
        Get-RegularDataPathFact $regularInstructions $badDelegatePool
    } 'datapath-delegate'

    $fallbackPool = @{
        1=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/form/export/xml/writer/FormChoiceParameterLinkWriter.getDataPathSegments:(Lx;)Ljava/lang/String;'
    }
    $fallbackInstruction = @(New-TestInstruction 73 'invokespecial' '#1')
    Assert-ExtensionFallbackInstruction $fallbackInstruction $fallbackPool
    $badFallbackPool = @{
        1=New-TestCpEntry 'Methodref' 'com/example/OtherWriter.getDataPathSegments:(Lx;)Ljava/lang/String;'
    }
    Assert-SelfTestRejected {
        Assert-ExtensionFallbackInstruction $fallbackInstruction $badFallbackPool
    } 'regular-extension-disagreement'

    $commonDefaultPool = @{
        1=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/metadata/common/impl/CommonPackageImpl.getAbstractChoiceParameterLink_Name:()Lorg/eclipse/emf/ecore/EAttribute;'
        2=New-TestCpEntry 'String' 'name'
        3=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/metadata/common/impl/CommonPackageImpl.initEAttribute:(Lx;)Lorg/eclipse/emf/ecore/EAttribute;'
        4=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/metadata/common/impl/CommonPackageImpl.getAbstractChoiceParameterLink_ChangeMode:()Lorg/eclipse/emf/ecore/EAttribute;'
        5=New-TestCpEntry 'Methodref' 'com/_1c/g5/v8/dt/metadata/common/impl/CommonPackageImpl.getLinkedValueChangeMode:()Lorg/eclipse/emf/ecore/EEnum;'
        6=New-TestCpEntry 'String' 'changeMode'
    }
    $commonOffsets = @(222,223,224,227,228,233,236,237,238,239,242,243,244,245,
        246,247,248,249,250,253,254,255,256,259,260,263,266,267,268,269,272,273,
        274,275,276,277,278,279,280,283)
    $commonOpcodes = @('aload_0','aload_0','invokevirtual','aload_1','invokeinterface',
        'ldc_w','aconst_null','iconst_1','iconst_1','ldc_w','iconst_0','iconst_0',
        'iconst_1','iconst_0','iconst_0','iconst_0','iconst_0','iconst_1',
        'invokevirtual','pop','aload_0','aload_0','invokevirtual','aload_0',
        'invokevirtual','ldc_w','aconst_null','iconst_0','iconst_1','ldc_w',
        'iconst_0','iconst_0','iconst_1','iconst_0','iconst_0','iconst_0',
        'iconst_0','iconst_1','invokevirtual','pop')
    $commonCpOffsets = @{ 224=1; 233=2; 250=3; 256=4; 260=5; 263=6; 280=3 }
    $commonRequired = [System.Collections.Generic.List[object]]::new()
    $commonRequired.Add((New-TestInstruction 4 'ifeq' '8'))
    for ($index = 0; $index -lt $commonOffsets.Count; $index++) {
        $offset = [int]$commonOffsets[$index]
        $operand = if ($commonCpOffsets.ContainsKey($offset)) {
            "#$($commonCpOffsets[$offset])"
        } else { '' }
        $commonRequired.Add((New-TestInstruction $offset $commonOpcodes[$index] $operand))
    }
    $commonRequired.Add((New-TestInstruction 6111 'return'))
    $commonDefaultFixture = @(New-SyntheticMethodInstructions @($commonRequired) 3040)
    $defaultFacts = Get-CommonChoiceParameterLinkDefaultFacts `
        $commonDefaultFixture $commonDefaultPool
    if ($null -ne $defaultFacts.name.modelDefault -or
        $null -ne $defaultFacts.changeMode.modelDefault) {
        throw 'Synthetic common-model positive fixture did not produce null defaults.'
    }
    $badNameDefault = @($commonDefaultFixture | ForEach-Object {
        if ([int]$_['offset'] -eq 236) {
            New-TestInstruction 236 'ldc' '#2'
        } else { $_ }
    })
    Assert-SelfTestRejected {
        Get-CommonChoiceParameterLinkDefaultFacts $badNameDefault $commonDefaultPool
    } 'name-model-default'
    $badChangeModeDefault = @($commonDefaultFixture | ForEach-Object {
        if ([int]$_['offset'] -eq 266) {
            New-TestInstruction 266 'ldc' '#6'
        } else { $_ }
    })
    Assert-SelfTestRejected {
        Get-CommonChoiceParameterLinkDefaultFacts $badChangeModeDefault $commonDefaultPool
    } 'change-mode-model-default'

    Write-Output 'ChoiceParameterLinks evidence extractor synthetic javap self-tests passed.'
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($VerifyDeterminism) {
    Assert-ExactRelease $EdtRelease
    $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        'ibcmd-choice-parameter-links-determinism-' + [guid]::NewGuid().ToString('N'))
    [System.IO.Directory]::CreateDirectory($temporaryRoot) | Out-Null
    try {
        $firstPath = Join-Path $temporaryRoot 'first.json'
        $secondPath = Join-Path $temporaryRoot 'second.json'
        $hostExecutable = (Get-Process -Id $PID).Path
        if ([string]::IsNullOrWhiteSpace($hostExecutable) -or
            -not (Test-Path -LiteralPath $hostExecutable -PathType Leaf)) {
            throw 'Unable to resolve the current PowerShell host for independent extraction runs.'
        }
        foreach ($candidate in @($firstPath, $secondPath)) {
            $childOutput = @(& $hostExecutable -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath `
                -InputInventory $InputInventory -OutputReport $candidate -EdtRelease $EdtRelease 2>&1)
            if ($LASTEXITCODE -ne 0) {
                throw "Independent extraction failed: $($childOutput -join "`n")"
            }
        }
        $firstBytes = [System.IO.File]::ReadAllBytes($firstPath)
        $secondBytes = [System.IO.File]::ReadAllBytes($secondPath)
        $firstHash = [System.BitConverter]::ToString(
            [System.Security.Cryptography.SHA256]::Create().ComputeHash($firstBytes)).Replace('-', '')
        $secondHash = [System.BitConverter]::ToString(
            [System.Security.Cryptography.SHA256]::Create().ComputeHash($secondBytes)).Replace('-', '')
        if ($firstHash -cne $secondHash -or $firstBytes.Length -ne $secondBytes.Length) {
            throw "Independent extraction outputs differ: $firstHash != $secondHash."
        }
        $fullOutput = [System.IO.Path]::GetFullPath($OutputReport)
        [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($fullOutput)) | Out-Null
        [System.IO.File]::WriteAllBytes($fullOutput, $firstBytes)
        Write-Output "Wrote $fullOutput from two independent extraction runs."
        Write-Output "deterministic-sha256=$firstHash"
    }
    finally {
        if (Test-Path -LiteralPath $temporaryRoot -PathType Container) {
            [System.IO.Directory]::Delete($temporaryRoot, $true)
        }
    }
    exit 0
}

if (-not (Get-Command javap -ErrorAction SilentlyContinue)) {
    throw 'javap is required for EDT ChoiceParameterLinks research extraction.'
}
Assert-ExactRelease $EdtRelease

$inventoryJson = Get-Content -LiteralPath $InputInventory -Raw
Assert-TopLevelInventoryArray $inventoryJson
$inventoryDocument = $inventoryJson | ConvertFrom-Json
$inventory = @($inventoryDocument | ForEach-Object { $_ })
if ($inventory.Count -eq 0 -or @($inventory | Where-Object {
    $null -eq $_.PSObject.Properties['bundle'] -or $null -eq $_.PSObject.Properties['jar']
}).Count -ne 0) {
    throw 'External inventory must be a top-level bundle array with bundle and local jar fields.'
}
$pluginDirectories = @($inventory | ForEach-Object {
    $jar = [string]$_.jar
    if (-not [string]::IsNullOrWhiteSpace($jar) -and (Test-Path -LiteralPath $jar -PathType Leaf)) {
        [System.IO.Path]::GetDirectoryName([System.IO.Path]::GetFullPath($jar))
    }
} | Sort-Object -Unique)
if ($pluginDirectories.Count -ne 1) {
    throw "Inventory must resolve to one installed EDT plugins directory; found $($pluginDirectories.Count)."
}

$bundles = [ordered]@{}
foreach ($name in $bundleContracts.Keys) {
    $bundles[$name] = Resolve-ResearchBundle $inventory $pluginDirectories $name
}

$classes = [ordered]@{
    itemWriter = @('com._1c.g5.v8.dt.form.export.xml',
        'com._1c.g5.v8.dt.form.export.xml.writer.FormChoiceParameterLinkWriter')
    orderProvider = @('com._1c.g5.v8.dt.form.export.xml',
        'com._1c.g5.v8.dt.form.export.xml.writer.ExtInfoWriter$ExtInfoFeatureOrderProvider')
    featureProvider = @('com._1c.g5.v8.dt.form.export.xml',
        'com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider')
    runtimeModule = @('com._1c.g5.v8.dt.form.export.xml',
        'com._1c.g5.v8.dt.form.export.xml.ExportFormXmlRuntimeModule')
    extensionWriter = @('com._1c.g5.v8.dt.form.extension.export.xml',
        'com._1c.g5.v8.dt.internal.form.extension.export.xml.writer.FormChoiceParameterLinkExtensionWriter')
    baseQName = @('com._1c.g5.v8.dt.export.xml',
        'com._1c.g5.v8.dt.export.xml.BaseQNameProvider')
    xr = @('com._1c.g5.v8.dt.export.xml',
        'com._1c.g5.v8.dt.export.xml.IXmlElements$XR')
    xsi = @('com._1c.g5.v8.dt.export.xml',
        'com._1c.g5.v8.dt.export.xml.IXmlElements$XSI')
    xs = @('com._1c.g5.v8.dt.export.xml',
        'com._1c.g5.v8.dt.export.xml.IXmlElements$XS')
    formPackage = @('com._1c.g5.v8.dt.form.model',
        'com._1c.g5.v8.dt.form.model.impl.FormPackageImpl')
    commonPackage = @('com._1c.g5.v8.dt.metadata',
        'com._1c.g5.v8.dt.metadata.common.impl.CommonPackageImpl')
    changeMode = @('com._1c.g5.v8.dt.metadata',
        'com._1c.g5.v8.dt.metadata.common.LinkedValueChangeMode')
}

$outputs = [ordered]@{}
foreach ($key in $classes.Keys) {
    $coordinate = $classes[$key]
    $outputs[$key] = @(Invoke-EdtJavap ([string]$bundles[$coordinate[0]]['jar']) $coordinate[1])
}
$pools = [ordered]@{}
foreach ($key in $outputs.Keys) { $pools[$key] = Get-JavapConstantPool $outputs[$key] }

$ownerInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.itemWriter '^  public void write\(' `
    '(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;)V'))
$itemInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.itemWriter '^  private void writeFormChoiceParameterLink\(' `
    '(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lcom/_1c/g5/v8/dt/form/model/FormChoiceParameterLink;Lcom/_1c/g5/v8/dt/form/model/Form;Lcom/_1c/g5/v8/dt/export/xml/IExportContext;)V'))
$regularInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.itemWriter '^  protected java.lang.String getDataPathSegments\(' `
    '(Lorg/eclipse/emf/ecore/EObject;Lcom/_1c/g5/v8/dt/form/model/AbstractDataPath;Lcom/_1c/g5/v8/dt/form/model/Form;)Ljava/lang/String;'))
$extensionInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.extensionWriter '^  protected java.lang.String getDataPathSegments\(' `
    '(Lorg/eclipse/emf/ecore/EObject;Lcom/_1c/g5/v8/dt/form/model/AbstractDataPath;Lcom/_1c/g5/v8/dt/form/model/Form;)Ljava/lang/String;'))
$orderInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.orderProvider '^  private java.util.List<org.eclipse.emf.ecore.EStructuralFeature> getInputFieldExtInfoFeatures\(' `
    '(Lcom/_1c/g5/v8/dt/platform/version/Version;)Ljava/util/List;'))
$formModelInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.formPackage '^  public void initializePackageContents\(\);$' '()V'))
$commonModelInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.commonPackage '^  public void initializePackageContents\(\);$' '()V'))

$ownerBehavior = Get-OwnerWriterFact $ownerInstructions $pools.itemWriter
$itemBehavior = Get-ItemWriterFact $itemInstructions $pools.itemWriter
$regularBehavior = Get-RegularDataPathFact $regularInstructions $pools.itemWriter
$extensionBehavior = Get-ExtensionFact $extensionInstructions $pools.extensionWriter
$orderBehavior = Get-OrderFact $orderInstructions $pools.orderProvider
$modelFacts = Get-ModelFacts $formModelInstructions $pools.formPackage `
    $commonModelInstructions $pools.commonPackage

Assert-ExactClassDeclaration $outputs.baseQName `
    'public class com._1c.g5.v8.dt.export.xml.BaseQNameProvider implements com._1c.g5.v8.dt.export.xml.IQNameProvider'
Assert-ExactClassDeclarationAndDeclaredMethods $outputs.featureProvider `
    'public class com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider extends com._1c.g5.v8.dt.export.xml.BaseQNameProvider' `
    @(
        '  public com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider();',
        '  protected void fillSpecifiedFeatureNames(com.google.common.collect.ImmutableMap$Builder<org.eclipse.emf.ecore.EStructuralFeature, javax.xml.namespace.QName>);',
        '  protected void fillSpecifiedPackageNsUri(com.google.common.collect.ImmutableMap$Builder<org.eclipse.emf.ecore.EPackage, java.lang.String>);'
    )
$baseConstructorInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.baseQName '^  public com\._1c\.g5\.v8\.dt\.export\.xml\.BaseQNameProvider\(\);$' '()V'))
Assert-MethodEnvelope $baseConstructorInstructions 30 @() 55 'return'
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 4 'invokestatic' 'Methodref' `
    'ImmutableMap\.builder:\(\)Lcom/google/common/collect/ImmutableMap\$Builder;$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 10 'invokevirtual' 'Methodref' `
    'BaseQNameProvider\.fillSpecifiedPackageNsUri:\(Lcom/google/common/collect/ImmutableMap\$Builder;\)V$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 15 'invokevirtual' 'Methodref' `
    'ImmutableMap\$Builder\.build:\(\)Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 18 'putfield' 'Fieldref' `
    'BaseQNameProvider\.specifiedPackageNS:Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 21 'invokestatic' 'Methodref' `
    'ImmutableMap\.builder:\(\)Lcom/google/common/collect/ImmutableMap\$Builder;$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 27 'invokevirtual' 'Methodref' `
    'BaseQNameProvider\.fillSpecifiedFeatureNames:\(Lcom/google/common/collect/ImmutableMap\$Builder;\)V$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 32 'invokevirtual' 'Methodref' `
    'ImmutableMap\$Builder\.build:\(\)Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $baseConstructorInstructions $pools.baseQName 35 'putfield' 'Fieldref' `
    'BaseQNameProvider\.specifiedFeatureNames:Lcom/google/common/collect/ImmutableMap;$' | Out-Null
$runtimeInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.runtimeModule `
    '^  public java.lang.Class<\? extends com\._1c\.g5\.v8\.dt\.export\.xml\.IQNameProvider> bindIQNameProvider\(\);$' `
    '()Ljava/lang/Class;'))
Assert-MethodEnvelope $runtimeInstructions 2 @() 2 'areturn'
Assert-CpInstruction $runtimeInstructions $pools.runtimeModule 0 'ldc' 'Class' `
    'com/_1c/g5/v8/dt/internal/form/export/xml/FormFeatureNameProvider$' | Out-Null

$elementQNameInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.baseQName '^  public javax.xml.namespace.QName getElementQName\(' `
    '(Lorg/eclipse/emf/ecore/EStructuralFeature;)Ljavax/xml/namespace/QName;'))
Assert-MethodEnvelope $elementQNameInstructions 15 @('8:ifeq:25','22:goto:30') 30 'areturn'
Assert-CpInstruction $elementQNameInstructions $pools.baseQName 1 'getfield' 'Fieldref' `
    'BaseQNameProvider\.specifiedFeatureNames:Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $elementQNameInstructions $pools.baseQName 5 'invokevirtual' 'Methodref' `
    'ImmutableMap\.containsKey:\(Ljava/lang/Object;\)Z$' | Out-Null
Assert-CpInstruction $elementQNameInstructions $pools.baseQName 12 'getfield' 'Fieldref' `
    'BaseQNameProvider\.specifiedFeatureNames:Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $elementQNameInstructions $pools.baseQName 16 'invokevirtual' 'Methodref' `
    'ImmutableMap\.get:\(Ljava/lang/Object;\)Ljava/lang/Object;$' | Out-Null
Assert-CpInstruction $elementQNameInstructions $pools.baseQName 27 'invokevirtual' 'Methodref' `
    'BaseQNameProvider\.capitalizeFirstLetter:\(Lorg/eclipse/emf/ecore/EStructuralFeature;\)Ljavax/xml/namespace/QName;$' | Out-Null
$capitalizeInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.baseQName '^  private javax.xml.namespace.QName capitalizeFirstLetter\(' `
    '(Lorg/eclipse/emf/ecore/EStructuralFeature;)Ljavax/xml/namespace/QName;'))
Assert-MethodEnvelope $capitalizeInstructions 24 @('4:ifeq:19','16:goto:25') 57 'areturn'
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 1 'invokevirtual' 'Methodref' `
    'BaseQNameProvider\.needToCapitalizeFirstLetterOfFeatureName:\(\)Z$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 8 'invokeinterface' 'InterfaceMethodref' `
    'EStructuralFeature\.getName:\(\)Ljava/lang/String;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 13 'invokestatic' 'Methodref' `
    'org/eclipse/xtext/util/Strings\.toFirstUpper:\(Ljava/lang/String;\)Ljava/lang/String;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 20 'invokeinterface' 'InterfaceMethodref' `
    'EStructuralFeature\.getName:\(\)Ljava/lang/String;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 27 'getfield' 'Fieldref' `
    'BaseQNameProvider\.specifiedPackageNS:Lcom/google/common/collect/ImmutableMap;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 31 'invokeinterface' 'InterfaceMethodref' `
    'EStructuralFeature\.getEContainingClass:\(\)Lorg/eclipse/emf/ecore/EClass;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 36 'invokeinterface' 'InterfaceMethodref' `
    'EClass\.getEPackage:\(\)Lorg/eclipse/emf/ecore/EPackage;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 41 'invokevirtual' 'Methodref' `
    'ImmutableMap\.get:\(Ljava/lang/Object;\)Ljava/lang/Object;$' | Out-Null
Assert-CpInstruction $capitalizeInstructions $pools.baseQName 54 'invokespecial' 'Methodref' `
    'javax/xml/namespace/QName\."<init>":\(Ljava/lang/String;Ljava/lang/String;\)V$' | Out-Null
$capitalizeFlag = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.baseQName '^  protected boolean needToCapitalizeFirstLetterOfFeatureName\(\);$' '()Z'))
Assert-MethodEnvelope $capitalizeFlag 2 @() 1 'ireturn'
Assert-Instruction (Get-InstructionAtOffset $capitalizeFlag 0) 'iconst_1'

$packageNamespaceInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.featureProvider '^  protected void fillSpecifiedPackageNsUri\(' `
    '(Lcom/google/common/collect/ImmutableMap$Builder;)V'))
Assert-MethodEnvelope $packageNamespaceInstructions 6 @() 11 'return'
Assert-CpInstruction $packageNamespaceInstructions $pools.featureProvider 1 'getstatic' 'Fieldref' `
    'FormPackage\.eINSTANCE:' | Out-Null
$formNamespace = Assert-CpInstruction $packageNamespaceInstructions $pools.featureProvider 4 `
    'ldc_w' 'String' '^http://v8\.1c\.ru/8\.3/xcf/logform$'
Assert-CpInstruction $packageNamespaceInstructions $pools.featureProvider 7 'invokevirtual' 'Methodref' `
    'com/google/common/collect/ImmutableMap\$Builder\.put:\(Ljava/lang/Object;Ljava/lang/Object;\)Lcom/google/common/collect/ImmutableMap\$Builder;$' | Out-Null
$featureNameInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.featureProvider '^  protected void fillSpecifiedFeatureNames\(' `
    '(Lcom/google/common/collect/ImmutableMap$Builder;)V'))
Assert-ExactFeatureNameMapEnvelope $featureNameInstructions $pools.featureProvider
Assert-FeatureQNameMapping $featureNameInstructions $pools.featureProvider `
    'ABSTRACT_CHOICE_PARAMETER_LINK__NAME:' 'IXmlElements\$XR\.NAME:'
Assert-FeatureQNameMapping $featureNameInstructions $pools.featureProvider `
    'ABSTRACT_CHOICE_PARAMETER_LINK__CHANGE_MODE:' 'IXmlElements\$XR\.VALUE_CHANGE:'
$ownerExplicitMappings = @($pools.featureProvider.Values | Where-Object {
    [string]$_['target'] -match 'INPUT_FIELD_EXT_INFO__CHOICE_PARAMETER_LINKS'
})
if ($ownerExplicitMappings.Count -ne 0) {
    throw 'Owner feature unexpectedly has an explicit FormFeatureNameProvider mapping.'
}
if ($modelFacts.ownerFeatureName -cne 'choiceParameterLinks') {
    throw "Owner model feature name '$($modelFacts.ownerFeatureName)' is not accepted."
}

$xrStatic = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock $outputs.xr '^  static \{\};$' '()V'))
$xsiStatic = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock $outputs.xsi '^  static \{\};$' '()V'))
$xsStatic = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock $outputs.xs '^  static \{\};$' '()V'))
$linkQName = Get-QNameFromStaticInitializer $xrStatic $pools.xr 'IXmlElements\$XR' 'LINK' `
    'http://v8.1c.ru/8.3/xcf/readable' 'Link' 'xr'
$dataPathQName = Get-QNameFromStaticInitializer $xrStatic $pools.xr 'IXmlElements\$XR' 'DATA_PATH' `
    'http://v8.1c.ru/8.3/xcf/readable' 'DataPath' 'xr'
$nameQName = Get-QNameFromStaticInitializer $xrStatic $pools.xr 'IXmlElements\$XR' 'NAME' `
    'http://v8.1c.ru/8.3/xcf/readable' 'Name' 'xr'
$changeQName = Get-QNameFromStaticInitializer $xrStatic $pools.xr 'IXmlElements\$XR' 'VALUE_CHANGE' `
    'http://v8.1c.ru/8.3/xcf/readable' 'ValueChange' 'xr'
$xsiTypeQName = Get-QNameFromStaticInitializer $xsiStatic $pools.xsi 'IXmlElements\$XSI' 'TYPE' `
    'http://www.w3.org/2001/XMLSchema-instance' 'type' 'xsi'
$xsStringQName = Get-QNameFromStaticInitializer $xsStatic $pools.xs 'IXmlElements\$XS' 'STRING' `
    'http://www.w3.org/2001/XMLSchema' 'string' 'xs'

$changeStatic = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.changeMode '^  static \{\};$' '()V'))
$toStringInstructions = @(ConvertTo-JavapInstructions (Get-VerifiedJavapMethodBlock `
    $outputs.changeMode '^  public java.lang.String toString\(\);$' '()Ljava/lang/String;'))
$lexicalMap = Get-LexicalMap $changeStatic $pools.changeMode $toStringInstructions

$wrapperLocal = [System.Char]::ToUpperInvariant($modelFacts.ownerFeatureName[0]) +
    $modelFacts.ownerFeatureName.Substring(1)
if ($wrapperLocal -cne 'ChoiceParameterLinks') {
    throw "QName fallback produced unexpected owner local name '$wrapperLocal'."
}
$wrapperQName = "{$formNamespace}$wrapperLocal"

function Get-PortableSource {
    param([string]$ClassKey, [string]$Member)
    $coordinate = $classes[$ClassKey]
    return "edt-derived://$EdtRelease/$($coordinate[0])/$($coordinate[1])#$Member"
}
$toolSource = 'tools/report-edt-form-choice-parameter-links-evidence.ps1'
$ownerEvidence = New-Evidence @(
    $toolSource,
    (Get-PortableSource 'itemWriter' 'write'),
    (Get-PortableSource 'orderProvider' 'getInputFieldExtInfoFeatures'),
    (Get-PortableSource 'runtimeModule' 'bindIQNameProvider'),
    (Get-PortableSource 'featureProvider' 'fillSpecifiedPackageNsUri/fillSpecifiedFeatureNames'),
    (Get-PortableSource 'baseQName' '<init>+getElementQName+capitalizeFirstLetter'),
    (Get-PortableSource 'formPackage' 'initializePackageContents'),
    (Get-PortableSource 'xr' '<clinit>')
) 'Exact owner model slice, runtime binding, BaseQNameProvider implements IQNameProvider hierarchy and constructor virtual fill calls, subclass hierarchy/declared-method absence of overrides, complete 286-instruction feature map, BaseQNameProvider map/fallback calls, method descriptors, writer branch graph, and provider predecessor/successor slice were parsed fail-closed.'
$itemEvidence = New-Evidence @(
    $toolSource,
    (Get-PortableSource 'itemWriter' 'writeFormChoiceParameterLink'),
    (Get-PortableSource 'featureProvider' 'fillSpecifiedFeatureNames'),
    (Get-PortableSource 'commonPackage' 'initializePackageContents'),
    (Get-PortableSource 'xr' '<clinit>')
) 'Exact method descriptors, 49-instruction item method, child order, feature mappings, QName initializers, and model default slices were parsed fail-closed.'
$dataPathEvidence = New-Evidence @(
    $toolSource,
    (Get-PortableSource 'itemWriter' 'writeFormChoiceParameterLink/getDataPathSegments'),
    (Get-PortableSource 'xr' '<clinit>'),
    (Get-PortableSource 'xsi' '<clinit>'),
    (Get-PortableSource 'xs' '<clinit>')
) 'Exact method descriptors, DataPath branch graph, direct regular delegate, dot Joiner, element QName, xsi:type QName and xs:string QName were parsed fail-closed.'
$extensionEvidence = New-Evidence @(
    $toolSource,
    (Get-PortableSource 'extensionWriter' 'getDataPathSegments'),
    (Get-PortableSource 'itemWriter' 'getDataPathSegments')
) 'Exact method descriptor, complete extension branch graph, sentinel branch, and invokespecial regular fallback were parsed fail-closed.'
$changeEvidence = New-Evidence @(
    $toolSource,
    (Get-PortableSource 'itemWriter' 'writeFormChoiceParameterLink'),
    (Get-PortableSource 'commonPackage' 'initializePackageContents'),
    (Get-PortableSource 'changeMode' '<clinit>+toString'),
    (Get-PortableSource 'xr' '<clinit>')
) 'Exact method descriptors, feature mapping/default, exact two-value enum initializer, literal-returning toString, and QName initializer were parsed fail-closed.'

$report = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = '1C:EDT'
        release = $EdtRelease
        derivation = 'research-only exact bundle manifests plus javap -v -p -c -constants class-hierarchy/method-descriptor/control-flow/constant-pool extraction; no JAR, class, bytecode, source, timestamp, or machine path retained'
        inputContract = 'external EDT Convector inventory must be a top-level JSON array identifying one exact installed EDT plugins directory; exact sibling metadata bundle is allowed because the inventory catalogs model exporters rather than every installed bundle'
        invocation = 'pwsh tools/report-edt-form-choice-parameter-links-evidence.ps1 -InputInventory <external-version-matched-inventory.json> -EdtRelease 2025.2.3+30 -OutputReport <portable-report.json> -VerifyDeterminism'
        bundles = @($bundles.Values | ForEach-Object {
            [ordered]@{ symbolicName = $_['name']; version = $_['version'] }
        })
    }
    scope = [ordered]@{
        disposition = 'research-only-recommendation-evidence'
        productionEmission = $false
    }
    verifiedFacts = @(
        [ordered]@{
            key = 'form.InputFieldExtInfo.choiceParameterLinks.owner-wrapper'
            value = [ordered]@{
                qname = $wrapperQName
                prefix = ''
                itemQName = $linkQName.qname
                itemPrefix = $linkQName.prefix
                qnameProviderChain = [ordered]@{
                    runtimeBinding = 'ExportFormXmlRuntimeModule.bindIQNameProvider -> FormFeatureNameProvider'
                    implementationSuperclass = 'FormFeatureNameProvider extends BaseQNameProvider'
                    baseContract = 'BaseQNameProvider implements IQNameProvider; constructor invokes virtual package/feature fill methods and stores immutable maps'
                    relevantSubclassOverride = 'absent-exact-declared-method-set'
                    ownerExplicitFeatureMapping = $false
                    fallback = 'BaseQNameProvider.getElementQName -> capitalizeFirstLetter -> QName(packageNamespace, firstUpper(featureName))'
                    packageNamespace = $formNamespace
                }
                model = [ordered]@{
                    default = $modelFacts.ownerModelDefault
                    lowerBound = $modelFacts.ownerLowerBound
                    upperBound = $modelFacts.ownerUpperBound
                }
                empty = $ownerBehavior.emptyCollection
                null = $ownerBehavior.null
                version = $orderBehavior.version
                order = [ordered]@{
                    predecessor = $orderBehavior.predecessor
                    successor = $orderBehavior.successor
                }
            }
            evidence = $ownerEvidence
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.verified-order'
            value = $itemBehavior.order
            evidence = $itemEvidence
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.name'
            value = [ordered]@{
                qname = $nameQName.qname
                prefix = $nameQName.prefix
                modelDefault = $modelFacts.name.modelDefault
                lowerBound = $modelFacts.name.lowerBound
                upperBound = $modelFacts.name.upperBound
                observedFeatureWriterBoolean = $itemBehavior.nameFeatureWriterBoolean
            }
            evidence = $itemEvidence
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.datapath'
            value = [ordered]@{
                qname = $dataPathQName.qname
                prefix = $dataPathQName.prefix
                null = $itemBehavior.datapathNull
                delegate = $regularBehavior.delegate
                segmentSource = $regularBehavior.segmentSource
                delimiter = $regularBehavior.delimiter
                xsiTypeAttributeQName = $xsiTypeQName.qname
                xsiTypeAttributePrefix = $xsiTypeQName.prefix
                xsiTypeValueQName = $xsStringQName.qname
                xsiTypeValuePrefix = $xsStringQName.prefix
            }
            evidence = $dataPathEvidence
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.changeMode'
            value = [ordered]@{
                qname = $changeQName.qname
                prefix = $changeQName.prefix
                modelDefault = $modelFacts.changeMode.modelDefault
                lowerBound = $modelFacts.changeMode.lowerBound
                upperBound = $modelFacts.changeMode.upperBound
                observedFeatureWriterBoolean = $itemBehavior.changeModeFeatureWriterBoolean
                lexicalMap = $lexicalMap
            }
            evidence = $changeEvidence
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.extension-datapath'
            value = $extensionBehavior
            evidence = $extensionEvidence
        }
    )
    missingKeys = @(
        [ordered]@{
            key = 'form.FormChoiceParameterLink.name.feature-writer-boolean-semantics'
            status = 'not-proven'
            reason = 'The item writer proves the raw false argument, but this extractor does not inspect every bound ISpecifiedElementWriter implementation needed to assign portable emission semantics.'
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.changeMode.feature-writer-boolean-semantics'
            status = 'not-proven'
            reason = 'The item writer proves the raw true argument and model default null, but the generic feature-writer boolean meaning is outside this slice.'
        },
        [ordered]@{
            key = 'form.FormChoiceParameterLink.datapath.semantic-resolution'
            status = 'not-proven'
            reason = 'The regular writer proves dot-joining AbstractDataPath segments and the extension sentinel branch; it does not prove broader runtime path resolution semantics.'
        },
        [ordered]@{
            key = 'form.InputFieldExtInfo.choiceParameterLinks.platform-version-range'
            status = 'not-proven'
            reason = 'The order-provider slice proves the feature is unconditional inside the inspected EDT method, not a historical minimum or future maximum platform version.'
        }
    )
}

$json = ConvertTo-DeterministicJson $report
Write-Utf8LfFile $OutputReport $json
Write-Output "Wrote $([System.IO.Path]::GetFullPath($OutputReport))"
Write-Output "verified=$($report.verifiedFacts.Count) missing=$($report.missingKeys.Count)"
