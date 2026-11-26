<#
.SYNOPSIS
Extracts portable, fail-closed evidence for EDT Form choice-list empty strings.

.DESCRIPTION
This research-only extractor accepts an installed, exact-release EDT root. It
retains no JAR, class, bytecode, source, or machine path. The report is emitted
only after exact method descriptors, instruction offsets/opcodes, control-flow
targets, and referenced constant-pool entries prove the complete delegate chain
down to XMLStreamWriter.writeEmptyElement.

.EXAMPLE
pwsh tools/report-edt-form-choice-list-string-writer-evidence.ps1 `
  -EdtRoot <installed-edt-root> `
  -EdtRelease 2025.2.3+30 `
  -OutputReport crates/ibcmd-schema/data/edt-2025.2.3-form-choice-list-string-writer-evidence.json

.EXAMPLE
pwsh tools/report-edt-form-choice-list-string-writer-evidence.ps1 `
  -SelfTest -EdtRoot <installed-edt-root>
#>
[CmdletBinding(DefaultParameterSetName = 'Extract')]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [Parameter(Mandatory = $true, ParameterSetName = 'SelfTest')]
    [string]$EdtRoot,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$OutputReport,

    [Parameter(Mandatory = $true, ParameterSetName = 'Extract')]
    [string]$EdtRelease,

    [Parameter(Mandatory = $true, ParameterSetName = 'SelfTest')]
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$formBundleName = 'com._1c.g5.v8.dt.form.export.xml'
$coreBundleName = 'com._1c.g5.v8.dt.export.xml'
$expectedEdtRelease = '2025.2.3+30'
$expectedEdtRootLeaf = '1c-edt-2025.2.3+30-x86_64'
$expectedProductVersion = '2025.2.3'
$expectedBuildId = '2025.2.3.30'
$expectedProduct = 'com._1c.g5.v8.dt.product.application.rcp'
$expectedApplication = 'org.eclipse.ui.ide.workbench'
$formBundleVersion = '10.1.0.v202602241426'
$coreBundleVersion = '13.0.100.v202602241426'
$choiceWriterClass = 'com._1c.g5.v8.dt.form.export.xml.writer.FormChoiceListDesTimeValueWriter'
$smartWriterClass = 'com._1c.g5.v8.dt.form.export.xml.writer.FormSmartFeatureWriter'
$formValueWriterClass = 'com._1c.g5.v8.dt.form.export.xml.writer.FormValueWriter'
$valueWriterClass = 'com._1c.g5.v8.dt.export.xml.writer.ValueWriter'
$streamWriterClass = 'com._1c.g5.v8.dt.export.xml.writer.ExportXmlStreamWriter'

function Invoke-EdtJavap {
    param(
        [Parameter(Mandatory)] [string]$Classpath,
        [Parameter(Mandatory)] [string]$ClassName
    )
    $lines = @(& javap -classpath $Classpath -v -p -c -constants $ClassName 2>&1)
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
        if ($Lines[$index] -match '^  (?:public |private |protected ).+\(.*;$' -or
            $Lines[$index] -match '^  static \{\};$') {
            $end = $index
            break
        }
    }
    return @($Lines[$start..($end - 1)])
}

function Assert-MethodDescriptor {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Block,
        [Parameter(Mandatory)] [string]$Descriptor
    )
    $descriptors = @($Block | Where-Object { $_ -match '^\s+descriptor:\s+(.+)$' })
    if ($descriptors.Count -ne 1 -or
        $descriptors[0] -notmatch ('^\s+descriptor:\s+' + [regex]::Escape($Descriptor) + '$')) {
        throw "Expected exact method descriptor '$Descriptor'."
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
    if ($result.Count -eq 0) {
        throw 'Method block has no parsed bytecode instructions.'
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
    $index = Get-ConstantPoolIndex $instruction
    if (-not $ConstantPool.ContainsKey($index)) {
        throw "Instruction at offset $Offset refers to absent constant-pool entry #$index."
    }
    $entry = $ConstantPool[$index]
    if ([string]$entry['kind'] -ne $Kind -or [string]$entry['target'] -notmatch $TargetPattern) {
        throw "Constant-pool proof failed at offset ${Offset}: '$($entry['text'])'."
    }
}

function Assert-ExactSequence {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] [int[]]$Offsets,
        [Parameter(Mandatory)] [string[]]$Opcodes
    )
    if ($Offsets.Count -ne $Opcodes.Count) {
        throw 'Internal extractor error: offset/opcode lengths differ.'
    }
    $first = -1
    for ($index = 0; $index -lt $Instructions.Count; $index++) {
        if ([int]$Instructions[$index]['offset'] -eq $Offsets[0]) {
            $first = $index
            break
        }
    }
    if ($first -lt 0 -or $first + $Offsets.Count -gt $Instructions.Count) {
        throw "Exact sequence beginning at $($Offsets[0]) is absent or truncated."
    }
    for ($index = 0; $index -lt $Offsets.Count; $index++) {
        $instruction = $Instructions[$first + $index]
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
    } | ForEach-Object {
        "$($_['offset']):$($_['opcode']):$($_['operand'])"
    })
}

function Assert-MethodEnvelope {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [object[]]$Instructions,
        [Parameter(Mandatory)] [int]$InstructionCount,
        [Parameter(Mandatory)] [int]$FirstOffset,
        [Parameter(Mandatory)] [int]$LastOffset,
        [Parameter(Mandatory)] [AllowEmptyCollection()] [string[]]$BranchGraph
    )
    if ($Instructions.Count -ne $InstructionCount) {
        throw "Instruction count is $($Instructions.Count), expected $InstructionCount."
    }
    if ([int]$Instructions[0]['offset'] -ne $FirstOffset -or
        [int]$Instructions[-1]['offset'] -ne $LastOffset) {
        throw "Method offset envelope is $($Instructions[0]['offset'])..$($Instructions[-1]['offset']), expected $FirstOffset..$LastOffset."
    }
    $actualBranches = @(Get-BranchGraph $Instructions)
    if ($actualBranches.Count -ne $BranchGraph.Count) {
        throw "Branch count is $($actualBranches.Count), expected $($BranchGraph.Count)."
    }
    for ($index = 0; $index -lt $BranchGraph.Count; $index++) {
        if ($actualBranches[$index] -cne $BranchGraph[$index]) {
            throw "Branch graph drift at index ${index}: '$($actualBranches[$index])', expected '$($BranchGraph[$index])'."
        }
    }
}

function Assert-ChoiceWriter {
    param($Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 108 0 253 @(
        '6:ifeq:21', '18:if_acmpeq:42', '73:ifne:242', '100:goto:225',
        '197:iflt:217', '232:ifne:103', '239:goto:253', '244:ifeq:253'
    )
    Assert-ExactSequence $Instructions `
        @(140, 141, 144, 147, 148, 151, 153, 156, 157, 160, 161, 163, 166, 167, 169,
          172, 173, 176, 177, 179, 182, 184, 186) `
        @('aload_1', 'getstatic', 'invokevirtual', 'aload_1', 'getstatic', 'ldc',
          'invokevirtual', 'aload_0', 'getfield', 'aload_1', 'aload', 'getstatic',
          'iconst_1', 'aload', 'invokevirtual', 'aload_0', 'getfield', 'aload_1',
          'aload', 'getstatic', 'iload', 'aload', 'invokevirtual')
    Assert-CpInstruction $Instructions $Pool 141 'getstatic' 'Fieldref' 'IXmlElements\$XR\.VALUE:'
    Assert-CpInstruction $Instructions $Pool 144 'invokevirtual' 'Methodref' 'ExportXmlStreamWriter\.writeStartElement:'
    Assert-CpInstruction $Instructions $Pool 148 'getstatic' 'Fieldref' 'IXmlElements\$XSI\.TYPE:'
    Assert-CpInstruction $Instructions $Pool 151 'ldc' 'String' '^FormChoiceListDesTimeValue$'
    Assert-CpInstruction $Instructions $Pool 179 'getstatic' 'Fieldref' 'FORM_CHOICE_LIST_DES_TIME_VALUE__VALUE:'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 182) 'iload' '4'
    Assert-CpInstruction $Instructions $Pool 186 'invokevirtual' 'Methodref' `
        'FormSmartFeatureWriter\.write:\(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;\)V'
}

function Assert-SmartWriter {
    param($WriteInstructions, $ClassifierInstructions, $Pool)
    Assert-MethodEnvelope $WriteInstructions 90 0 209 @(
        '8:ifeq:49', '46:goto:209', '53:if_acmpeq:122', '60:if_acmpeq:122',
        '76:ifeq:122', '119:goto:209', '134:ifeq:164', '161:goto:209',
        '168:ifeq:188', '185:goto:209', '192:ifeq:209'
    )
    Assert-MethodEnvelope $ClassifierInstructions 165 0 360 @()
    Assert-ExactSequence $WriteInstructions `
        @(63, 64, 67, 68, 73, 76, 79, 80, 83, 84, 87, 88, 93, 96, 99, 104,
          107, 108, 109, 110, 112, 114, 119) `
        @('aload_0', 'getfield', 'aload_3', 'invokeinterface', 'invokevirtual', 'ifeq',
          'aload_0', 'getfield', 'aload_0', 'getfield', 'aload_3', 'invokeinterface',
          'invokevirtual', 'checkcast', 'invokeinterface', 'checkcast', 'aload_1',
          'aload_2', 'aload_3', 'iload', 'aload', 'invokeinterface', 'goto')
    Assert-Instruction (Get-InstructionAtOffset $WriteInstructions 76) 'ifeq' '122'
    Assert-CpInstruction $WriteInstructions $Pool 64 'getfield' 'Fieldref' `
        'FormSmartFeatureWriter\.specifiedClassifierWriters:Lcom/google/common/collect/ImmutableMap;'
    Assert-CpInstruction $WriteInstructions $Pool 68 'invokeinterface' 'InterfaceMethodref' 'EStructuralFeature\.getEType:'
    Assert-CpInstruction $WriteInstructions $Pool 73 'invokevirtual' 'Methodref' `
        'ImmutableMap\.containsKey:\(Ljava/lang/Object;\)Z'
    Assert-CpInstruction $WriteInstructions $Pool 80 'getfield' 'Fieldref' `
        'FormSmartFeatureWriter\.injector:Lcom/google/inject/Injector;'
    Assert-CpInstruction $WriteInstructions $Pool 84 'getfield' 'Fieldref' `
        'FormSmartFeatureWriter\.specifiedClassifierWriters:Lcom/google/common/collect/ImmutableMap;'
    Assert-CpInstruction $WriteInstructions $Pool 88 'invokeinterface' 'InterfaceMethodref' 'EStructuralFeature\.getEType:'
    Assert-CpInstruction $WriteInstructions $Pool 93 'invokevirtual' 'Methodref' `
        'ImmutableMap\.get:\(Ljava/lang/Object;\)Ljava/lang/Object;'
    Assert-CpInstruction $WriteInstructions $Pool 99 'invokeinterface' 'InterfaceMethodref' `
        'com/google/inject/Injector\.getInstance:\(Ljava/lang/Class;\)Ljava/lang/Object;'
    Assert-CpInstruction $WriteInstructions $Pool 104 'checkcast' 'Class' `
        '^com/_1c/g5/v8/dt/export/xml/writer/ISpecifiedElementWriter$'
    Assert-CpInstruction $WriteInstructions $Pool 114 'invokeinterface' 'InterfaceMethodref' `
        'ISpecifiedElementWriter\.write:\(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;\)V'

    Assert-ExactSequence $ClassifierInstructions @(268, 269, 272, 275, 278) `
        @('aload_1', 'getstatic', 'ldc_w', 'invokevirtual', 'pop')
    Assert-CpInstruction $ClassifierInstructions $Pool 269 'getstatic' 'Fieldref' `
        'McorePackage\$Literals\.VALUE:Lorg/eclipse/emf/ecore/EClass;'
    Assert-CpInstruction $ClassifierInstructions $Pool 272 'ldc_w' 'Class' `
        '^com/_1c/g5/v8/dt/form/export/xml/writer/FormValueWriter$'
    Assert-CpInstruction $ClassifierInstructions $Pool 275 'invokevirtual' 'Methodref' `
        'ImmutableMap\$Builder\.put:\(Ljava/lang/Object;Ljava/lang/Object;\)Lcom/google/common/collect/ImmutableMap\$Builder;'
}

function Assert-FormValueWriter {
    param($Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 125 0 314 @(
        '4:ifeq:301', '16:ifnull:301', '59:ifne:279', '80:ifeq:252',
        '90:ifnull:108', '105:ifeq:115', '112:goto:252', '125:ifeq:252',
        '196:ifnull:214', '211:goto:240', '245:ifeq:252', '276:goto:314',
        '281:ifeq:314', '298:goto:314'
    )
    Assert-ExactSequence $Instructions @(301, 302, 303, 304, 305, 307, 309, 311, 314) `
        @('aload_0', 'aload_1', 'aload_2', 'aload_3', 'iload', 'aload', 'aload',
          'invokespecial', 'return')
    Assert-CpInstruction $Instructions $Pool 311 'invokespecial' 'Methodref' `
        'ValueWriter\.writeValue:\(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Ljava/lang/Object;Ljavax/xml/namespace/QName;ZLorg/eclipse/emf/ecore/EStructuralFeature;Lcom/_1c/g5/v8/dt/export/xml/IExportContext;\)V'
}

function Assert-CoreValueWriter {
    param($Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 567 0 1345 @(
        '1:ifnonnull:5', '9:ifeq:29', '26:goto:1345', '33:ifeq:53',
        '50:goto:1345', '57:ifeq:98', '95:goto:1345', '102:ifeq:143',
        '140:goto:1345', '147:ifeq:215', '166:ifeq:187', '184:goto:1345',
        '212:goto:1345', '219:ifeq:260', '257:goto:1345', '264:ifeq:302',
        '299:goto:1345', '306:ifeq:433', '330:ifnonnull:355', '335:ifeq:1345',
        '352:goto:1345', '381:ifne:411', '408:goto:1345', '413:ifeq:1345',
        '430:goto:1345', '437:ifeq:508', '505:goto:1345', '512:ifeq:640',
        '533:ifne:622', '565:goto:605', '612:ifne:568', '619:goto:1345',
        '637:goto:1345', '644:ifeq:735', '678:goto:718', '725:ifne:681',
        '732:goto:1345', '739:ifeq:832', '774:ifne:809', '806:goto:1345',
        '811:ifeq:1345', '829:goto:1345', '836:ifeq:886', '883:goto:1345',
        '890:ifeq:994', '963:iflt:987', '991:goto:1345', '998:ifeq:1023',
        '1020:goto:1345', '1027:ifeq:1055', '1052:goto:1345',
        '1059:ifeq:1095', '1092:goto:1345', '1099:ifeq:1140',
        '1137:goto:1345', '1144:ifeq:1189', '1186:goto:1345',
        '1193:ifeq:1246', '1209:ifnull:1345', '1243:goto:1345',
        '1250:ifeq:1317', '1266:ifnull:1345', '1282:if_icmpeq:1345',
        '1314:goto:1345'
    )
    Assert-ExactSequence $Instructions `
        @(143, 144, 147, 150, 151, 154, 159, 161, 163, 166, 169, 170, 171,
          174, 175, 178, 181, 184, 187, 188, 189, 192, 193, 196, 199, 202,
          203, 205, 208, 209, 212, 215) `
        @('aload_2', 'instanceof', 'ifeq', 'aload_2', 'checkcast', 'invokeinterface',
          'astore', 'aload', 'invokestatic', 'ifeq', 'aload_1', 'aload_3',
          'invokevirtual', 'aload_1', 'getstatic', 'getstatic', 'invokevirtual',
          'goto', 'aload_1', 'aload_3', 'invokevirtual', 'aload_1', 'getstatic',
          'getstatic', 'invokevirtual', 'aload_1', 'aload', 'invokevirtual',
          'aload_1', 'invokevirtual', 'goto', 'aload_2')
    Assert-Instruction (Get-InstructionAtOffset $Instructions 147) 'ifeq' '215'
    Assert-Instruction (Get-InstructionAtOffset $Instructions 166) 'ifeq' '187'
    Assert-CpInstruction $Instructions $Pool 144 'instanceof' 'Class' `
        '^com/_1c/g5/v8/dt/mcore/StringValue$'
    Assert-CpInstruction $Instructions $Pool 154 'invokeinterface' 'InterfaceMethodref' `
        'StringValue\.getValue:\(\)Ljava/lang/String;'
    Assert-CpInstruction $Instructions $Pool 163 'invokestatic' 'Methodref' `
        'Strings\.isNullOrEmpty:\(Ljava/lang/String;\)Z'
    Assert-CpInstruction $Instructions $Pool 171 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeEmptyElement:\(Ljavax/xml/namespace/QName;\)V'
    Assert-CpInstruction $Instructions $Pool 175 'getstatic' 'Fieldref' 'IXmlElements\$XSI\.TYPE:'
    Assert-CpInstruction $Instructions $Pool 178 'getstatic' 'Fieldref' 'IXmlElements\$XS\.STRING:'
    Assert-CpInstruction $Instructions $Pool 181 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeAttribute:\(Ljavax/xml/namespace/QName;Ljavax/xml/namespace/QName;\)V'
    Assert-CpInstruction $Instructions $Pool 189 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeStartElement:\(Ljavax/xml/namespace/QName;\)V'
    Assert-CpInstruction $Instructions $Pool 205 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeCharacters:\(Ljava/lang/String;\)V'
    Assert-CpInstruction $Instructions $Pool 209 'invokevirtual' 'Methodref' `
        'ExportXmlStreamWriter\.writeInlineEndElement:\(\)V'
}

function Assert-StreamWriter {
    param($Instructions, $Pool)
    Assert-MethodEnvelope $Instructions 21 0 42 @('14:ifeq:21')
    Assert-ExactSequence $Instructions `
        @(0, 1, 4, 5, 6, 9, 10, 11, 14, 17, 18, 21, 22, 25, 26, 29, 30,
          33, 34, 37, 42) `
        @('aload_0', 'invokevirtual', 'aload_0', 'aload_1', 'invokevirtual', 'astore_1',
          'aload_0', 'getfield', 'ifeq', 'aload_0', 'invokevirtual', 'aload_0',
          'getfield', 'aload_1', 'invokevirtual', 'aload_1', 'invokevirtual',
          'aload_1', 'invokevirtual', 'invokeinterface', 'return')
    Assert-Instruction (Get-InstructionAtOffset $Instructions 14) 'ifeq' '21'
    Assert-CpInstruction $Instructions $Pool 37 'invokeinterface' 'InterfaceMethodref' `
        'javax/xml/stream/XMLStreamWriter\.writeEmptyElement:\(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;\)V'
}

function New-TestInstruction {
    param([int]$Offset, [string]$Opcode, [string]$Operand = '')
    return [ordered]@{ offset = $Offset; opcode = $Opcode; operand = $Operand; comment = ''; text = "$Offset`: $Opcode $Operand" }
}

function Copy-TestInstructions {
    param([Parameter(Mandatory)] [object[]]$Instructions)
    return @($Instructions | ForEach-Object {
        [ordered]@{
            offset = [int]$_['offset']
            opcode = [string]$_['opcode']
            operand = [string]$_['operand']
            comment = [string]$_['comment']
            text = [string]$_['text']
        }
    })
}

function Copy-TestPool {
    param([Parameter(Mandatory)] $Pool)
    $copy = @{}
    foreach ($key in $Pool.Keys) {
        $copy[$key] = [ordered]@{
            kind = [string]$Pool[$key]['kind']
            target = [string]$Pool[$key]['target']
            text = [string]$Pool[$key]['text']
        }
    }
    return $copy
}

function Set-TestCpTarget {
    param(
        [Parameter(Mandatory)] [object[]]$Instructions,
        [Parameter(Mandatory)] $Pool,
        [Parameter(Mandatory)] [int]$Offset,
        [Parameter(Mandatory)] [string]$Target
    )
    $instruction = Get-InstructionAtOffset $Instructions $Offset
    $index = Get-ConstantPoolIndex $instruction
    if (-not $Pool.ContainsKey($index)) {
        throw "Self-test constant-pool entry #$index is absent."
    }
    $Pool[$index]['target'] = $Target
    $Pool[$index]['text'] = "$($Pool[$index]['kind']) // $Target"
}

function Assert-SelfTestRejected {
    param([Parameter(Mandatory)] [scriptblock]$Action, [Parameter(Mandatory)] [string]$Name)
    try {
        & $Action | Out-Null
    }
    catch {
        return
    }
    throw "Self-test '$Name' was not rejected."
}

function Invoke-SelfTest {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$ChoiceBlock,
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$SmartWriteBlock,
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$ClassifierBlock,
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$FormValueBlock,
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$ValueBlock,
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$StreamBlock,
        [Parameter(Mandatory)] [string]$FeatureDescriptor,
        [Parameter(Mandatory)] [string]$ValueDescriptor,
        [Parameter(Mandatory)] [object[]]$ChoiceInstructions,
        [Parameter(Mandatory)] [object[]]$SmartWriteInstructions,
        [Parameter(Mandatory)] [object[]]$ClassifierInstructions,
        [Parameter(Mandatory)] [object[]]$FormValueInstructions,
        [Parameter(Mandatory)] [object[]]$ValueInstructions,
        [Parameter(Mandatory)] [object[]]$StreamInstructions,
        [Parameter(Mandatory)] $ChoicePool,
        [Parameter(Mandatory)] $SmartPool,
        [Parameter(Mandatory)] $FormValuePool,
        [Parameter(Mandatory)] $ValuePool,
        [Parameter(Mandatory)] $StreamPool,
        [Parameter(Mandatory)] [string]$ConfigText
    )

    Assert-MethodDescriptor $ChoiceBlock $FeatureDescriptor
    Assert-MethodDescriptor $SmartWriteBlock $FeatureDescriptor
    Assert-MethodDescriptor $ClassifierBlock '()Lcom/google/common/collect/ImmutableMap;'
    Assert-MethodDescriptor $FormValueBlock $ValueDescriptor
    Assert-MethodDescriptor $ValueBlock $ValueDescriptor
    Assert-MethodDescriptor $StreamBlock '(Ljavax/xml/namespace/QName;)V'
    Assert-ChoiceWriter $ChoiceInstructions $ChoicePool
    Assert-SmartWriter $SmartWriteInstructions $ClassifierInstructions $SmartPool
    Assert-FormValueWriter $FormValueInstructions $FormValuePool
    Assert-CoreValueWriter $ValueInstructions $ValuePool
    Assert-StreamWriter $StreamInstructions $StreamPool
    Assert-ExactEdtIdentityText $expectedEdtRootLeaf $ConfigText $expectedEdtRelease

    Assert-SelfTestRejected {
        Assert-ExactEdtIdentityText $expectedEdtRootLeaf $ConfigText '2025.2.3+29'
    } 'wrong-release'
    Assert-SelfTestRejected {
        Assert-ExactEdtIdentityText 'copied-edt-root' $ConfigText $expectedEdtRelease
    } 'wrong-root-identity'
    $wrongConfig = $ConfigText -replace 'eclipse\.buildId=2025\.2\.3\.30', 'eclipse.buildId=2025.2.3.29'
    Assert-SelfTestRejected {
        Assert-ExactEdtIdentityText $expectedEdtRootLeaf $wrongConfig $expectedEdtRelease
    } 'wrong-build-id'

    $validManifest = @{
        'Bundle-SymbolicName' = "$formBundleName;singleton:=true"
        'Bundle-Version' = $formBundleVersion
    }
    Assert-BundleManifestValues $validManifest $formBundleName $formBundleVersion
    $wrongManifest = $validManifest.Clone()
    $wrongManifest['Bundle-Version'] = '10.1.0.v202602241425'
    Assert-SelfTestRejected {
        Assert-BundleManifestValues $wrongManifest $formBundleName $formBundleVersion
    } 'wrong-bundle-version'

    $badDescriptor = @($ChoiceBlock | ForEach-Object {
        $_ -replace [regex]::Escape($FeatureDescriptor), '()V'
    })
    Assert-SelfTestRejected {
        Assert-MethodDescriptor $badDescriptor $FeatureDescriptor
    } 'perturbed-method-descriptor'

    $shortChoice = @($ChoiceInstructions[0..($ChoiceInstructions.Count - 2)])
    Assert-SelfTestRejected {
        Assert-ChoiceWriter $shortChoice $ChoicePool
    } 'choice-method-envelope'

    $badSmartPool = Copy-TestPool $SmartPool
    Set-TestCpTarget $ClassifierInstructions $badSmartPool 275 `
        'com/google/common/collect/ImmutableMap$Builder.putAll:(Ljava/util/Map;)Lcom/google/common/collect/ImmutableMap$Builder;'
    Assert-SelfTestRejected {
        Assert-SmartWriter $SmartWriteInstructions $ClassifierInstructions $badSmartPool
    } 'smart-classifier-map-put'

    $badFormValuePool = Copy-TestPool $FormValuePool
    Set-TestCpTarget $FormValueInstructions $badFormValuePool 311 `
        'com/_1c/g5/v8/dt/export/xml/writer/ValueWriter.writeOther:(Ljava/lang/Object;)V'
    Assert-SelfTestRejected {
        Assert-FormValueWriter $FormValueInstructions $badFormValuePool
    } 'form-value-super-delegate'

    $badValueBranch = Copy-TestInstructions $ValueInstructions
    (Get-InstructionAtOffset $badValueBranch 166)['operand'] = '188'
    Assert-SelfTestRejected {
        Assert-CoreValueWriter $badValueBranch $ValuePool
    } 'core-empty-branch'

    $badValuePool = Copy-TestPool $ValuePool
    Set-TestCpTarget $ValueInstructions $badValuePool 181 `
        'com/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter.writeStartElement:(Ljavax/xml/namespace/QName;)V'
    Assert-SelfTestRejected {
        Assert-CoreValueWriter $ValueInstructions $badValuePool
    } 'core-xsi-attribute-after-empty'

    $badValueOrder = Copy-TestInstructions $ValueInstructions
    (Get-InstructionAtOffset $badValueOrder 181)['offset'] = 180
    Assert-SelfTestRejected {
        Assert-CoreValueWriter $badValueOrder $ValuePool
    } 'core-empty-then-attribute-order'

    $badStreamPool = Copy-TestPool $StreamPool
    Set-TestCpTarget $StreamInstructions $badStreamPool 37 `
        'javax/xml/stream/XMLStreamWriter.writeStartElement:(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V'
    Assert-SelfTestRejected {
        Assert-StreamWriter $StreamInstructions $badStreamPool
    } 'terminal-xml-stream-empty-element'

    Write-Output 'Form choice-list string writer evidence extractor self-tests passed.'
}

function ConvertTo-DeterministicJson {
    param([Parameter(Mandatory)] $Value)
    return (($Value | ConvertTo-Json -Depth 16 -Compress).Replace("`r`n", "`n") + "`n")
}

function Write-Utf8LfFile {
    param([Parameter(Mandatory)] [string]$Path, [Parameter(Mandatory)] [string]$Text)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($fullPath)) | Out-Null
    [System.IO.File]::WriteAllText($fullPath, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Get-ExactConfigValue {
    param(
        [Parameter(Mandatory)] [string]$Text,
        [Parameter(Mandatory)] [string]$Key
    )
    $pattern = '^' + [regex]::Escape($Key) + '=(.*)$'
    $matches = @($Text -split "`r?`n" | Where-Object { $_ -match $pattern })
    if ($matches.Count -ne 1 -or $matches[0] -notmatch $pattern) {
        throw "Expected exactly one '$Key' identity entry."
    }
    return [string]$Matches[1]
}

function Assert-ExactEdtIdentityText {
    param(
        [Parameter(Mandatory)] [string]$RootLeaf,
        [Parameter(Mandatory)] [string]$ConfigText,
        [Parameter(Mandatory)] [string]$Release
    )
    if ($Release -cne $expectedEdtRelease) {
        throw "Only exact EDT release '$expectedEdtRelease' is supported."
    }
    if ($RootLeaf -cne $expectedEdtRootLeaf) {
        throw "EDT root identity '$RootLeaf' is not '$expectedEdtRootLeaf'."
    }
    $expected = [ordered]@{
        'product.version' = $expectedProductVersion
        'eclipse.buildId' = $expectedBuildId
        'eclipse.product' = $expectedProduct
        'eclipse.application' = $expectedApplication
    }
    foreach ($entry in $expected.GetEnumerator()) {
        $actual = Get-ExactConfigValue $ConfigText $entry.Key
        if ($actual -cne [string]$entry.Value) {
            throw "EDT identity '$($entry.Key)' is '$actual', expected '$($entry.Value)'."
        }
    }
}

function Get-JarManifestHeaders {
    param([Parameter(Mandatory)] [string]$Jar)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($Jar)
    try {
        $entry = $archive.GetEntry('META-INF/MANIFEST.MF')
        if ($null -eq $entry) {
            throw 'Bundle JAR has no META-INF/MANIFEST.MF.'
        }
        $reader = [System.IO.StreamReader]::new($entry.Open())
        try {
            $raw = $reader.ReadToEnd()
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $archive.Dispose()
    }
    $unfolded = $raw -replace "`r?`n ", ''
    $headers = @{}
    foreach ($line in ($unfolded -split "`r?`n")) {
        if ($line.Length -eq 0) {
            break
        }
        if ($line -notmatch '^([^:]+):\s?(.*)$') {
            continue
        }
        $key = [string]$Matches[1]
        if ($headers.ContainsKey($key)) {
            throw "Duplicate manifest header '$key'."
        }
        $headers[$key] = [string]$Matches[2]
    }
    return $headers
}

function Assert-BundleManifestValues {
    param(
        [Parameter(Mandatory)] $Headers,
        [Parameter(Mandatory)] [string]$Bundle,
        [Parameter(Mandatory)] [string]$Version
    )
    foreach ($key in @('Bundle-SymbolicName', 'Bundle-Version')) {
        if (-not $Headers.ContainsKey($key)) {
            throw "Bundle manifest has no '$key' header."
        }
    }
    $symbolicName = ([string]$Headers['Bundle-SymbolicName'] -split ';', 2)[0]
    if ($symbolicName -cne $Bundle -or [string]$Headers['Bundle-Version'] -cne $Version) {
        throw "Bundle manifest identity does not match exact '$Bundle' '$Version'."
    }
}

function Get-ExactBundleJar {
    param(
        [Parameter(Mandatory)] [string]$Bundle,
        [Parameter(Mandatory)] [string]$Version
    )
    $matches = @(Get-ChildItem -LiteralPath $plugins -Filter "$Bundle`_*.jar" -File)
    if ($matches.Count -ne 1) {
        throw "Expected exactly one installed '$Bundle' JAR; found $($matches.Count)."
    }
    $expectedFileName = "$Bundle`_$Version.jar"
    if ($matches[0].Name -cne $expectedFileName) {
        throw "Bundle filename '$($matches[0].Name)' is not exact '$expectedFileName'."
    }
    $headers = Get-JarManifestHeaders $matches[0].FullName
    Assert-BundleManifestValues $headers $Bundle $Version
    return $matches[0].FullName
}

if (-not (Get-Command javap -ErrorAction SilentlyContinue)) {
    throw 'javap is required for EDT Form writer research extraction.'
}
$effectiveRelease = if ($SelfTest) { $expectedEdtRelease } else { $EdtRelease }
$resolvedEdtRoot = (Resolve-Path -LiteralPath $EdtRoot).Path
$configPath = Join-Path $resolvedEdtRoot 'configuration/config.ini'
if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
    throw 'EdtRoot must contain configuration/config.ini.'
}
$configText = Get-Content -LiteralPath $configPath -Raw
Assert-ExactEdtIdentityText ([System.IO.Path]::GetFileName($resolvedEdtRoot)) $configText $effectiveRelease
$plugins = Join-Path $resolvedEdtRoot 'plugins'
if (-not (Test-Path -LiteralPath $plugins -PathType Container)) {
    throw 'EdtRoot must contain a plugins directory.'
}
$formJar = Get-ExactBundleJar $formBundleName $formBundleVersion
$coreJar = Get-ExactBundleJar $coreBundleName $coreBundleVersion
$formClasspath = $formJar
$combinedClasspath = $formJar + [System.IO.Path]::PathSeparator + $coreJar

$choiceLines = @(Invoke-EdtJavap $formClasspath $choiceWriterClass)
$smartLines = @(Invoke-EdtJavap $formClasspath $smartWriterClass)
$formValueLines = @(Invoke-EdtJavap $combinedClasspath $formValueWriterClass)
$valueLines = @(Invoke-EdtJavap $coreJar $valueWriterClass)
$streamLines = @(Invoke-EdtJavap $coreJar $streamWriterClass)

$featureDescriptor = '(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;)V'
$valueDescriptor = '(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Ljava/lang/Object;Ljavax/xml/namespace/QName;ZLorg/eclipse/emf/ecore/EStructuralFeature;Lcom/_1c/g5/v8/dt/export/xml/IExportContext;)V'
$choiceBlock = Get-JavapMethodBlock $choiceLines '^  public void write\('
$smartWriteBlock = Get-JavapMethodBlock $smartLines '^  public void write\('
$classifierBlock = Get-JavapMethodBlock $smartLines '^  private com\.google\.common\.collect\.ImmutableMap<.+> fillSpecialClassifierWriters\(\);$'
$formValueBlock = Get-JavapMethodBlock $formValueLines '^  public void writeValue\('
$valueBlock = Get-JavapMethodBlock $valueLines '^  public void writeValue\('
$streamBlock = Get-JavapMethodBlock $streamLines '^  public void writeEmptyElement\(javax\.xml\.namespace\.QName\)'

Assert-MethodDescriptor $choiceBlock $featureDescriptor
Assert-MethodDescriptor $smartWriteBlock $featureDescriptor
Assert-MethodDescriptor $classifierBlock '()Lcom/google/common/collect/ImmutableMap;'
Assert-MethodDescriptor $formValueBlock $valueDescriptor
Assert-MethodDescriptor $valueBlock $valueDescriptor
Assert-MethodDescriptor $streamBlock '(Ljavax/xml/namespace/QName;)V'
if (@($formValueLines | Where-Object {
    $_ -eq 'public class com._1c.g5.v8.dt.form.export.xml.writer.FormValueWriter extends com._1c.g5.v8.dt.export.xml.writer.ValueWriter'
}).Count -ne 1) {
    throw 'FormValueWriter exact superclass relationship was not proven.'
}

$choicePool = Get-JavapConstantPool $choiceLines
$smartPool = Get-JavapConstantPool $smartLines
$formValuePool = Get-JavapConstantPool $formValueLines
$valuePool = Get-JavapConstantPool $valueLines
$streamPool = Get-JavapConstantPool $streamLines
$choiceInstructions = @(ConvertTo-JavapInstructions $choiceBlock)
$smartWriteInstructions = @(ConvertTo-JavapInstructions $smartWriteBlock)
$classifierInstructions = @(ConvertTo-JavapInstructions $classifierBlock)
$formValueInstructions = @(ConvertTo-JavapInstructions $formValueBlock)
$valueInstructions = @(ConvertTo-JavapInstructions $valueBlock)
$streamInstructions = @(ConvertTo-JavapInstructions $streamBlock)

Assert-ChoiceWriter $choiceInstructions $choicePool
Assert-SmartWriter $smartWriteInstructions $classifierInstructions $smartPool
Assert-FormValueWriter $formValueInstructions $formValuePool
Assert-CoreValueWriter $valueInstructions $valuePool
Assert-StreamWriter $streamInstructions $streamPool

if ($SelfTest) {
    Invoke-SelfTest `
        -ChoiceBlock $choiceBlock `
        -SmartWriteBlock $smartWriteBlock `
        -ClassifierBlock $classifierBlock `
        -FormValueBlock $formValueBlock `
        -ValueBlock $valueBlock `
        -StreamBlock $streamBlock `
        -FeatureDescriptor $featureDescriptor `
        -ValueDescriptor $valueDescriptor `
        -ChoiceInstructions $choiceInstructions `
        -SmartWriteInstructions $smartWriteInstructions `
        -ClassifierInstructions $classifierInstructions `
        -FormValueInstructions $formValueInstructions `
        -ValueInstructions $valueInstructions `
        -StreamInstructions $streamInstructions `
        -ChoicePool $choicePool `
        -SmartPool $smartPool `
        -FormValuePool $formValuePool `
        -ValuePool $valuePool `
        -StreamPool $streamPool `
        -ConfigText $configText
    exit 0
}

$sources = @(
    'tools/report-edt-form-choice-list-string-writer-evidence.ps1',
    "edt-derived://$effectiveRelease/$formBundleName/$choiceWriterClass#write$featureDescriptor",
    "edt-derived://$effectiveRelease/$formBundleName/$smartWriterClass#write$featureDescriptor",
    "edt-derived://$effectiveRelease/$formBundleName/$smartWriterClass#fillSpecialClassifierWriters()Lcom/google/common/collect/ImmutableMap;",
    "edt-derived://$effectiveRelease/$formBundleName/$formValueWriterClass#writeValue$valueDescriptor",
    "edt-derived://$effectiveRelease/$coreBundleName/$valueWriterClass#writeValue$valueDescriptor",
    "edt-derived://$effectiveRelease/$coreBundleName/$streamWriterClass#writeEmptyElement(Ljavax/xml/namespace/QName;)V"
)
$report = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = '1C:EDT'
        release = $effectiveRelease
        rootIdentity = [ordered]@{
            leaf = $expectedEdtRootLeaf
            productVersion = $expectedProductVersion
            buildId = $expectedBuildId
            product = $expectedProduct
            application = $expectedApplication
        }
        validatedBundles = @(
            [ordered]@{ symbolicName = $formBundleName; version = $formBundleVersion },
            [ordered]@{ symbolicName = $coreBundleName; version = $coreBundleVersion }
        )
        derivation = 'research-only installed exact-release bundles; javap -v -p -c -constants exact descriptor/control-flow/constant-pool extraction; no JAR, bytecode, source, or machine path retained'
        inputContract = 'EdtRoot must identify an installed exact-release EDT containing exactly one Form XML export bundle and one core XML export bundle'
        invocation = 'pwsh tools/report-edt-form-choice-list-string-writer-evidence.ps1 -EdtRoot <installed-exact-release-edt-root> -EdtRelease <release> -OutputReport <portable-report.json>'
    }
    verifiedFacts = @(
        [ordered]@{
            key = 'form.FormChoiceListDesTimeValue.value.empty-string'
            value = [ordered]@{
                modelValueType = 'mcore:StringValue'
                emptyPredicate = 'Strings.isNullOrEmpty'
                element = 'feature QName'
                xsiType = 'xs:string'
                emission = 'self-closing'
                delegateChain = @(
                    'FormChoiceListDesTimeValueWriter.write',
                    'FormSmartFeatureWriter.write',
                    'FormValueWriter.writeValue',
                    'ValueWriter.writeValue',
                    'ExportXmlStreamWriter.writeEmptyElement',
                    'XMLStreamWriter.writeEmptyElement'
                )
                branch = [ordered]@{
                    stringTypeOffset = 144
                    emptyPredicateOffset = 163
                    nonEmptyTargetOffset = 187
                    emptyElementOffset = 171
                    xsiTypeAttributeOffset = 181
                }
                methodEnvelopes = @(
                    [ordered]@{
                        method = 'FormChoiceListDesTimeValueWriter.write'
                        descriptor = $featureDescriptor
                        instructionCount = $choiceInstructions.Count
                        firstOffset = 0
                        lastOffset = 253
                        branchGraph = @(Get-BranchGraph $choiceInstructions)
                    },
                    [ordered]@{
                        method = 'FormSmartFeatureWriter.write'
                        descriptor = $featureDescriptor
                        instructionCount = $smartWriteInstructions.Count
                        firstOffset = 0
                        lastOffset = 209
                        branchGraph = @(Get-BranchGraph $smartWriteInstructions)
                    },
                    [ordered]@{
                        method = 'FormSmartFeatureWriter.fillSpecialClassifierWriters'
                        descriptor = '()Lcom/google/common/collect/ImmutableMap;'
                        instructionCount = $classifierInstructions.Count
                        firstOffset = 0
                        lastOffset = 360
                        branchGraph = @(Get-BranchGraph $classifierInstructions)
                    },
                    [ordered]@{
                        method = 'FormValueWriter.writeValue'
                        descriptor = $valueDescriptor
                        instructionCount = $formValueInstructions.Count
                        firstOffset = 0
                        lastOffset = 314
                        branchGraph = @(Get-BranchGraph $formValueInstructions)
                    },
                    [ordered]@{
                        method = 'ValueWriter.writeValue'
                        descriptor = $valueDescriptor
                        instructionCount = $valueInstructions.Count
                        firstOffset = 0
                        lastOffset = 1345
                        branchGraph = @(Get-BranchGraph $valueInstructions)
                    },
                    [ordered]@{
                        method = 'ExportXmlStreamWriter.writeEmptyElement'
                        descriptor = '(Ljavax/xml/namespace/QName;)V'
                        instructionCount = $streamInstructions.Count
                        firstOffset = 0
                        lastOffset = 42
                        branchGraph = @(Get-BranchGraph $streamInstructions)
                    }
                )
            }
            evidence = [ordered]@{
                kind = 'javap-v-exact-method-control-flow-constant-pool'
                status = 'verified'
                sources = $sources
                note = 'Exact descriptors, complete instruction-count/branch-graph envelopes, instruction sequences, and constant-pool targets prove the classifier map dispatch, full value-feature delegate chain, writeEmptyElement then xsi:type attribute order, and terminal XMLStreamWriter.writeEmptyElement call.'
            }
        }
    )
    missingKeys = @()
}
$jsonFirst = ConvertTo-DeterministicJson $report
$jsonSecond = ConvertTo-DeterministicJson $report
if ($jsonFirst -cne $jsonSecond) {
    throw 'Form choice-list string writer evidence report generation is nondeterministic.'
}
Write-Utf8LfFile $OutputReport $jsonFirst
Write-Output "Wrote $([System.IO.Path]::GetFullPath($OutputReport))"
Write-Output "verified=$($report.verifiedFacts.Count) missing=$($report.missingKeys.Count)"
