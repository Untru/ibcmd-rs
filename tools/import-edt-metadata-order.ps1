[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputInventory,

    [Parameter(Mandatory = $true)]
    [string]$OutputOrder,

    [string]$RejectReport,

    [string]$EdtRelease = "2025.2.3+30"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$bundleName = 'com._1c.g5.v8.dt.md.export.xml'
$providerClass = 'com._1c.g5.v8.dt.internal.md.export.xml.writer.MetadataObjectFeatureOrderProvider'
$producedTypesProviderClass = 'com._1c.g5.v8.dt.internal.md.export.xml.writer.ProducedTypesOrderProvider'
$providerInternalName = $providerClass.Replace('.', '/')
$providerEvidenceSource = "edt-derived://$EdtRelease/$bundleName/$providerClass"
$innerInfoFallback = 'eClass.getEStructuralFeature("producedTypes") when present, otherwise empty list'

function Invoke-EdtJavap {
    param(
        [Parameter(Mandatory)] [string]$Jar,
        [Parameter(Mandatory)] [string]$ClassName
    )

    # Verbose output is required: the ordinary -c output identifies only an
    # InvokeDynamic slot, while BootstrapMethods contains the method handle
    # that proves which get* implementation that slot invokes.
    $lines = @(& javap -classpath $Jar -v -p -c -constants $ClassName 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "javap failed for ${ClassName}: $($lines -join "`n")"
    }
    return $lines
}

function Get-FieldLiteralToken {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string]$Line)

    if ($Line -match 'Literals\.([A-Z0-9_]+):Lorg/eclipse/emf/ecore/E(?:Class|Reference|StructuralFeature|Attribute);') {
        return $Matches[1]
    }
    return $null
}

function Get-ProducedTypesRecords {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] [string]$Fallback
    )

    # The only accepted static-table shape is a single stack-linear
    # ImmutableMap.Builder expression.  Every array length/index/store and
    # every call is validated; marker scanning is deliberately forbidden.
    $records = [System.Collections.Generic.List[object]]::new()
    $rejected = [System.Collections.Generic.List[object]]::new()
    $block = Get-JavapMethodBlock -Lines $Lines -HeaderPattern '^  static \{\};$'
    $instructions = @(ConvertTo-JavapInstructions -Lines $block)
    if ($instructions.Count -lt 15) {
        throw 'ProducedTypes ORDER_MAP static initializer is shorter than the accepted shape.'
    }
    Assert-Instruction -Instruction $instructions[0] -Opcode 'new' -CommentPattern 'ImmutableMap\$Builder'
    Assert-Instruction -Instruction $instructions[1] -Opcode 'dup'
    Assert-Instruction -Instruction $instructions[2] -Opcode 'invokespecial' -CommentPattern 'ImmutableMap\$Builder\."<init>"'

    $cursor = 3
    $tailStart = $instructions.Count - 3
    while ($cursor -lt $tailStart) {
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EClass;'
        $classifier = Get-FieldLiteralToken -Line ([string]$instructions[$cursor]['comment'])
        if ($null -eq $classifier) {
            throw "ProducedTypes table key is not a literal EClass at offset $($instructions[$cursor]['offset'])."
        }
        $cursor++
        $arrayLength = Get-JvmLiteralInteger -Instruction $instructions[$cursor]
        if ($arrayLength -le 0) {
            throw "ProducedTypes table '$classifier' has a non-positive literal array length."
        }
        $cursor++
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'anewarray' -CommentPattern 'org/eclipse/emf/ecore/EReference'
        $cursor++

        $features = [System.Collections.Generic.List[string]]::new()
        for ($arrayIndex = 0; $arrayIndex -lt $arrayLength; $arrayIndex++) {
            Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'dup'
            $cursor++
            $observedIndex = Get-JvmLiteralInteger -Instruction $instructions[$cursor]
            if ($observedIndex -ne $arrayIndex) {
                throw "ProducedTypes table '$classifier' array index $observedIndex is not the expected $arrayIndex."
            }
            $cursor++
            Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EReference;'
            $feature = Get-FieldLiteralToken -Line ([string]$instructions[$cursor]['comment'])
            if ($null -eq $feature) {
                throw "ProducedTypes table '$classifier' contains a nonliteral EReference."
            }
            $features.Add($feature)
            $cursor++
            Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'aastore'
            $cursor++
        }
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'invokestatic' -CommentPattern 'com/google/common/collect/Lists\.newArrayList:'
        $cursor++
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'invokevirtual' -CommentPattern 'ImmutableMap\$Builder\.put:'
        $cursor++

        $records.Add([ordered]@{
            provider = 'ProducedTypesOrderProvider'
            classifier = $classifier
            section = 'producedTypes'
            orderedFeatures = @($features)
            versionPredicate = 'always'
            fallback = $Fallback
            evidence = [ordered]@{
                status = 'verified'
                kind = 'javap-v-exact-static-eclass-ereference-array-table'
                sources = @(
                    'tools/import-edt-metadata-order.ps1',
                    "edt-derived://$EdtRelease/$bundleName/$producedTypesProviderClass#static-initializer",
                    "edt-derived://$EdtRelease/$bundleName/$producedTypesProviderClass#getOrderedReferences"
                )
                note = 'complete stack-linear EClass key, literal indexed EReference array, Lists.newArrayList and ImmutableMap.Builder.put shape; fallback independently proved from getOrderedReferences'
            }
        })
    }
    if ($cursor -ne $tailStart) {
        throw 'ProducedTypes ORDER_MAP entries do not end on the accepted instruction boundary.'
    }
    Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'invokevirtual' -CommentPattern 'ImmutableMap\$Builder\.build:'
    Assert-Instruction -Instruction $instructions[$cursor + 1] -Opcode 'putstatic' -CommentPattern 'ORDER_MAP:'
    Assert-Instruction -Instruction $instructions[$cursor + 2] -Opcode 'return'
    if ($records.Count -eq 0) {
        throw 'ProducedTypes ORDER_MAP contains no verified entries.'
    }

    return [ordered]@{ records = @($records); rejected = @($rejected) }
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
        # javap method declarations have exactly two leading spaces and end in
        # ");".  Attributes and bytecode have deeper indentation.
        if ($Lines[$index] -match '^  (?:public |private |protected )?(?:static )?.+\);$') {
            $end = $index
            break
        }
        if ($Lines[$index] -match '^  static \{\};$') {
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

function Assert-Instruction {
    param(
        [Parameter(Mandatory)] $Instruction,
        [Parameter(Mandatory)] [string]$Opcode,
        [string]$CommentPattern
    )

    if ([string]$Instruction['opcode'] -ne $Opcode) {
        throw "Expected opcode '$Opcode', found '$($Instruction['text'])'."
    }
    if (-not [string]::IsNullOrWhiteSpace($CommentPattern) -and
        [string]$Instruction['comment'] -notmatch $CommentPattern) {
        throw "Instruction '$($Instruction['text'])' does not match evidence pattern '$CommentPattern'."
    }
}

function Get-JvmLiteralInteger {
    param([Parameter(Mandatory)] $Instruction)

    $opcode = [string]$Instruction['opcode']
    if ($opcode -match '^iconst_([0-5])$') {
        return [int]$Matches[1]
    }
    if ($opcode -eq 'iconst_m1') {
        return -1
    }
    if ($opcode -eq 'bipush' -or $opcode -eq 'sipush') {
        if ([string]$Instruction['operand'] -match '^(-?\d+)$') {
            return [int]$Matches[1]
        }
    }
    throw "Expected a literal JVM integer push, found '$($Instruction['text'])'."
}

function Get-InvokeDynamicBootstrapMap {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $map = @{}
    foreach ($line in $Lines) {
        if ($line -match '^\s+#(\d+) = InvokeDynamic\s+#(\d+):#\d+\s+') {
            $constantPoolIndex = [int]$Matches[1]
            $bootstrapIndex = [int]$Matches[2]
            if ($map.ContainsKey($constantPoolIndex)) {
                throw "Duplicate InvokeDynamic constant-pool index #$constantPoolIndex."
            }
            $map[$constantPoolIndex] = $bootstrapIndex
        }
    }
    if ($map.Count -eq 0) {
        throw 'No InvokeDynamic constant-pool entries found in verbose javap output.'
    }
    return $map
}

function Get-ProviderBootstrapMemberMap {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $bootstrapStart = -1
    for ($index = 0; $index -lt $Lines.Count; $index++) {
        if ($Lines[$index] -eq 'BootstrapMethods:') {
            $bootstrapStart = $index + 1
            break
        }
    }
    if ($bootstrapStart -lt 0) {
        throw 'Verbose javap output has no BootstrapMethods attribute.'
    }

    $targets = @{}
    $currentIndex = $null
    $isLambdaMetafactory = $false
    $methodTargets = [System.Collections.Generic.List[object]]::new()

    function Complete-BootstrapEntry {
        if ($null -eq $currentIndex) {
            return
        }
        if ($isLambdaMetafactory -and $methodTargets.Count -eq 1) {
            $targets[[int]$currentIndex] = $methodTargets[0]
        } elseif ($isLambdaMetafactory -and $methodTargets.Count -gt 1) {
            throw "BootstrapMethods entry $currentIndex has ambiguous metadata-provider targets."
        }
    }

    for ($index = $bootstrapStart; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        if ($line -match '^(InnerClasses|NestMembers|SourceFile):') {
            break
        }
        if ($line -match '^\s+(\d+):\s+#\d+\s+REF_invokeStatic\s+java/lang/invoke/LambdaMetafactory\.metafactory:') {
            Complete-BootstrapEntry
            $currentIndex = [int]$Matches[1]
            $isLambdaMetafactory = $true
            $methodTargets = [System.Collections.Generic.List[object]]::new()
            continue
        }
        if ($line -match '^\s+(\d+):') {
            Complete-BootstrapEntry
            $currentIndex = [int]$Matches[1]
            $isLambdaMetafactory = $false
            $methodTargets = [System.Collections.Generic.List[object]]::new()
            continue
        }
        if ($null -ne $currentIndex -and $isLambdaMetafactory -and
            $line -match ('REF_invoke(Special|Virtual|Static)\s+' +
                [regex]::Escape($providerInternalName) +
                '\.([A-Za-z0-9_$<>]+):(.+)$')) {
            $methodTargets.Add([ordered]@{
                handleKind = [string]$Matches[1]
                method = [string]$Matches[2]
                descriptor = [string]$Matches[3]
            })
        }
    }
    Complete-BootstrapEntry

    if ($targets.Count -eq 0) {
        throw 'No provider LambdaMetafactory method handles found in BootstrapMethods.'
    }
    return $targets
}

function Get-LambdaBootstrapTargetMap {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $members = Get-ProviderBootstrapMemberMap -Lines $Lines
    $targets = @{}
    foreach ($bootstrapIndex in $members.Keys) {
        $member = $members[$bootstrapIndex]
        if ([string]$member['method'] -match '^get[A-Za-z0-9]+$' -and
            [string]$member['descriptor'] -eq '(Lcom/_1c/g5/v8/dt/platform/version/Version;)Ljava/util/List;') {
            $targets[[int]$bootstrapIndex] = [string]$member['method']
        }
    }
    if ($targets.Count -eq 0) {
        throw 'No version-to-list get* handles found in provider BootstrapMethods.'
    }
    return $targets
}

function Resolve-InvokeDynamicMember {
    param(
        [Parameter(Mandatory)] $Instruction,
        [Parameter(Mandatory)] $ConstantPoolToBootstrap,
        [Parameter(Mandatory)] $BootstrapToMember
    )

    Assert-Instruction -Instruction $Instruction -Opcode 'invokedynamic' -CommentPattern 'InvokeDynamic #\d+:'
    if ([string]$Instruction['operand'] -notmatch '^#(\d+),') {
        throw "InvokeDynamic has an unknown constant-pool operand: $($Instruction['text'])."
    }
    $constantPoolIndex = [int]$Matches[1]
    if (-not $ConstantPoolToBootstrap.ContainsKey($constantPoolIndex)) {
        throw "InvokeDynamic CP #$constantPoolIndex has no parsed bootstrap index."
    }
    $bootstrapIndex = [int]$ConstantPoolToBootstrap[$constantPoolIndex]
    if ([string]$Instruction['comment'] -notmatch '^InvokeDynamic #(\d+):' -or
        [int]$Matches[1] -ne $bootstrapIndex) {
        throw "InvokeDynamic CP #$constantPoolIndex disagrees with its disassembly bootstrap index."
    }
    if (-not $BootstrapToMember.ContainsKey($bootstrapIndex)) {
        throw "BootstrapMethods entry $bootstrapIndex has no unique provider member target."
    }
    return [ordered]@{
        constantPoolIndex = $constantPoolIndex
        bootstrapIndex = $bootstrapIndex
        member = $BootstrapToMember[$bootstrapIndex]
    }
}

function Get-PropertiesFallbackProof {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $constantPoolToBootstrap = Get-InvokeDynamicBootstrapMap -Lines $Lines
    $bootstrapToMember = Get-ProviderBootstrapMemberMap -Lines $Lines
    $block = Get-JavapMethodBlock -Lines $Lines -HeaderPattern (
        '^  public java\.util\.List<org\.eclipse\.emf\.ecore\.EStructuralFeature> getProperties\(')
    $instructions = @(ConvertTo-JavapInstructions -Lines $block)
    $expectedOpcodes = @(
        'aload_0', 'getfield', 'aload_1', 'aload_0', 'aload_1',
        'invokedynamic', 'invokeinterface', 'checkcast', 'aload_2',
        'invokeinterface', 'invokeinterface', 'checkcast', 'areturn'
    )
    if ($instructions.Count -ne $expectedOpcodes.Count) {
        throw 'getProperties has an unknown instruction count.'
    }
    for ($index = 0; $index -lt $expectedOpcodes.Count; $index++) {
        Assert-Instruction -Instruction $instructions[$index] -Opcode $expectedOpcodes[$index]
    }
    Assert-Instruction -Instruction $instructions[1] -Opcode 'getfield' -CommentPattern 'propertiesOrderMap:Ljava/util/Map;'
    Assert-Instruction -Instruction $instructions[6] -Opcode 'invokeinterface' -CommentPattern 'java/util/Map\.getOrDefault:'
    Assert-Instruction -Instruction $instructions[9] -Opcode 'invokeinterface' -CommentPattern 'IExportContext\.getProjectVersion:'
    Assert-Instruction -Instruction $instructions[10] -Opcode 'invokeinterface' -CommentPattern 'java/util/function/Function\.apply:'

    $fallbackLambda = Resolve-InvokeDynamicMember -Instruction $instructions[5] `
        -ConstantPoolToBootstrap $constantPoolToBootstrap `
        -BootstrapToMember $bootstrapToMember
    $fallbackMember = $fallbackLambda['member']
    if ([string]$fallbackMember['method'] -ne 'lambda$39' -or
        [string]$fallbackMember['descriptor'] -ne '(Lorg/eclipse/emf/ecore/EClass;Lcom/_1c/g5/v8/dt/platform/version/Version;)Ljava/util/List;') {
        throw 'getProperties map-miss LambdaMetafactory target is not the accepted EClass fallback builder.'
    }

    $lambdaBlock = Get-JavapMethodBlock -Lines $Lines -HeaderPattern (
        '^  private java\.util\.List lambda\$39\(org\.eclipse\.emf\.ecore\.EClass, ')
    $lambdaInstructions = @(ConvertTo-JavapInstructions -Lines $lambdaBlock)
    $lambdaOpcodes = @(
        'new', 'dup', 'aload_1', 'aload_0', 'invokedynamic',
        'invokespecial', 'invokevirtual', 'areturn'
    )
    if ($lambdaInstructions.Count -ne $lambdaOpcodes.Count) {
        throw 'getProperties fallback lambda has an unknown instruction count.'
    }
    for ($index = 0; $index -lt $lambdaOpcodes.Count; $index++) {
        Assert-Instruction -Instruction $lambdaInstructions[$index] -Opcode $lambdaOpcodes[$index]
    }
    Assert-Instruction -Instruction $lambdaInstructions[0] -Opcode 'new' -CommentPattern 'MetadataObjectFeatureOrderProvider\$ListBuilder'
    Assert-Instruction -Instruction $lambdaInstructions[5] -Opcode 'invokespecial' -CommentPattern 'ListBuilder\."<init>":'
    Assert-Instruction -Instruction $lambdaInstructions[6] -Opcode 'invokevirtual' -CommentPattern 'ListBuilder\.build:'

    $filterLambda = Resolve-InvokeDynamicMember -Instruction $lambdaInstructions[4] `
        -ConstantPoolToBootstrap $constantPoolToBootstrap `
        -BootstrapToMember $bootstrapToMember
    $filterMember = $filterLambda['member']
    if ([string]$filterMember['method'] -ne 'defaultPropertyFilter' -or
        [string]$filterMember['descriptor'] -ne '(Lorg/eclipse/emf/ecore/EStructuralFeature;)Z') {
        throw 'getProperties fallback ListBuilder predicate is not the proved defaultPropertyFilter target.'
    }

    return [ordered]@{
        value = 'ListBuilder(eClass, defaultPropertyFilter).build() when propertiesOrderMap has no key'
        note = "getProperties CP #$($fallbackLambda['constantPoolIndex']) / bootstrap $($fallbackLambda['bootstrapIndex']) resolves map miss to lambda`$39; its predicate CP #$($filterLambda['constantPoolIndex']) / bootstrap $($filterLambda['bootstrapIndex']) resolves to defaultPropertyFilter"
    }
}

function Get-ProducedTypesFallbackProof {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $block = Get-JavapMethodBlock -Lines $Lines -HeaderPattern (
        '^  public java\.util\.List<org\.eclipse\.emf\.ecore\.EReference> getOrderedReferences\(')
    $instructions = @(ConvertTo-JavapInstructions -Lines $block)
    $expectedOpcodes = @(
        'getstatic', 'aload_1', 'invokeinterface', 'invokeinterface',
        'checkcast', 'astore_2', 'aload_2', 'ifnull', 'aload_2', 'goto',
        'aload_1', 'invokeinterface', 'invokeinterface', 'areturn'
    )
    if ($instructions.Count -ne $expectedOpcodes.Count) {
        throw 'getOrderedReferences has an unknown instruction count.'
    }
    for ($index = 0; $index -lt $expectedOpcodes.Count; $index++) {
        Assert-Instruction -Instruction $instructions[$index] -Opcode $expectedOpcodes[$index]
    }
    Assert-Instruction -Instruction $instructions[0] -Opcode 'getstatic' -CommentPattern 'ORDER_MAP:Ljava/util/Map;'
    Assert-Instruction -Instruction $instructions[2] -Opcode 'invokeinterface' -CommentPattern 'MdTypes\.eClass:'
    Assert-Instruction -Instruction $instructions[3] -Opcode 'invokeinterface' -CommentPattern 'java/util/Map\.get:'
    Assert-Instruction -Instruction $instructions[11] -Opcode 'invokeinterface' -CommentPattern 'MdTypes\.eClass:'
    Assert-Instruction -Instruction $instructions[12] -Opcode 'invokeinterface' -CommentPattern 'EClass\.getEAllReferences:'
    if ([string]$instructions[7]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$instructions[10]['offset']) {
        throw 'getOrderedReferences null map value does not branch to eClass fallback.'
    }
    if ([string]$instructions[9]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$instructions[13]['offset']) {
        throw 'getOrderedReferences mapped and fallback values do not join at areturn.'
    }
    return [ordered]@{
        value = 'eClass.getEAllReferences() when ORDER_MAP has no key'
        note = 'exact ORDER_MAP.get null branch invokes mdTypes.eClass().getEAllReferences() and joins directly at areturn'
    }
}

function Get-ConstructorBindings {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] $ConstantPoolToBootstrap,
        [Parameter(Mandatory)] $BootstrapToMethod
    )

    $header = '^  public ' + [regex]::Escape($providerClass) + '\(\);$'
    $constructorBlock = Get-JavapMethodBlock -Lines $Lines -HeaderPattern $header
    $instructions = @(ConvertTo-JavapInstructions -Lines $constructorBlock)
    if ($instructions.Count -lt 17) {
        throw 'Metadata provider constructor is shorter than the accepted map-builder shape.'
    }

    $prologue = @('aload_0', 'invokespecial', 'new', 'dup', 'invokespecial', 'astore_1')
    for ($index = 0; $index -lt $prologue.Count; $index++) {
        Assert-Instruction -Instruction $instructions[$index] -Opcode $prologue[$index]
    }
    Assert-Instruction -Instruction $instructions[1] -Opcode 'invokespecial' -CommentPattern 'java/lang/Object\."<init>"'
    Assert-Instruction -Instruction $instructions[2] -Opcode 'new' -CommentPattern 'ImmutableMap\$Builder'
    Assert-Instruction -Instruction $instructions[4] -Opcode 'invokespecial' -CommentPattern 'ImmutableMap\$Builder\."<init>"'

    $bindings = [System.Collections.Generic.List[object]]::new()
    $cursor = $prologue.Count
    while ($cursor -lt $instructions.Count - 5) {
        $group = @($instructions[$cursor..($cursor + 5)])
        Assert-Instruction -Instruction $group[0] -Opcode 'aload_1'
        Assert-Instruction -Instruction $group[1] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EClass;'
        Assert-Instruction -Instruction $group[2] -Opcode 'aload_0'
        Assert-Instruction -Instruction $group[3] -Opcode 'invokedynamic' -CommentPattern 'InvokeDynamic #\d+:apply:'
        Assert-Instruction -Instruction $group[4] -Opcode 'invokevirtual' -CommentPattern 'ImmutableMap\$Builder\.put:'
        Assert-Instruction -Instruction $group[5] -Opcode 'pop'

        $classifier = Get-FieldLiteralToken -Line ([string]$group[1]['comment'])
        if ($null -eq $classifier) {
            throw "Constructor EClass instruction has no literal classifier: $($group[1]['text'])."
        }
        if ([string]$group[3]['operand'] -notmatch '^#(\d+),') {
            throw "Constructor InvokeDynamic has an unknown operand: $($group[3]['text'])."
        }
        $constantPoolIndex = [int]$Matches[1]
        if (-not $ConstantPoolToBootstrap.ContainsKey($constantPoolIndex)) {
            throw "InvokeDynamic constant-pool index #$constantPoolIndex has no parsed bootstrap index."
        }
        $bootstrapIndex = [int]$ConstantPoolToBootstrap[$constantPoolIndex]
        if ([string]$group[3]['comment'] -notmatch '^InvokeDynamic #(\d+):apply:') {
            throw "Constructor InvokeDynamic comment has no bootstrap index: $($group[3]['text'])."
        }
        if ([int]$Matches[1] -ne $bootstrapIndex) {
            throw "InvokeDynamic #$constantPoolIndex disagrees with its comment bootstrap index."
        }
        if (-not $BootstrapToMethod.ContainsKey($bootstrapIndex)) {
            throw "BootstrapMethods entry $bootstrapIndex has no unique provider get* target."
        }

        $bindings.Add([ordered]@{
            classifier = $classifier
            method = [string]$BootstrapToMethod[$bootstrapIndex]
            constantPoolIndex = $constantPoolIndex
            bootstrapIndex = $bootstrapIndex
        })
        $cursor += 6
    }

    if ($cursor -ne $instructions.Count - 5) {
        throw 'Metadata provider constructor map entries do not end on the accepted instruction boundary.'
    }
    $epilogue = @($instructions[$cursor..($cursor + 4)])
    Assert-Instruction -Instruction $epilogue[0] -Opcode 'aload_0'
    Assert-Instruction -Instruction $epilogue[1] -Opcode 'aload_1'
    Assert-Instruction -Instruction $epilogue[2] -Opcode 'invokevirtual' -CommentPattern 'ImmutableMap\$Builder\.build:'
    Assert-Instruction -Instruction $epilogue[3] -Opcode 'putfield' -CommentPattern 'propertiesOrderMap:'
    Assert-Instruction -Instruction $epilogue[4] -Opcode 'return'

    $classifiers = @{}
    $methods = @{}
    foreach ($binding in $bindings) {
        if ($classifiers.ContainsKey([string]$binding['classifier'])) {
            throw "Duplicate constructor classifier '$($binding['classifier'])'."
        }
        if ($methods.ContainsKey([string]$binding['method'])) {
            throw "Constructor method '$($binding['method'])' is bound more than once."
        }
        $classifiers[[string]$binding['classifier']] = $true
        $methods[[string]$binding['method']] = $true
    }
    return @($bindings)
}

function New-OrderOperation {
    param(
        [Parameter(Mandatory)] [string]$Operation,
        [Parameter(Mandatory)] [string]$Feature
    )
    return [ordered]@{ operation = $Operation; feature = $Feature }
}

function Get-LinearListBuilderOperations {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [object[]]$Instructions,
        [Parameter(Mandatory)] [string]$Classifier
    )

    $branch = @($Instructions | Where-Object {
        [string]$_['opcode'] -match '^(if|goto|tableswitch|lookupswitch|jsr)'
    })
    if ($branch.Count -ne 0) {
        throw "method contains unsupported control flow: $([string]::Join(', ', @($branch | ForEach-Object { $_['text'] })))"
    }

    # Accept only the two observed, stack-linear builders.  This rejects any
    # additional call, local mutation, or other bytecode even when it has no
    # explicit branch.
    $pairStart = 0
    if ([string]$Instructions[0]['opcode'] -eq 'new') {
        if ($Instructions.Count -lt 10) {
            throw 'ordinary ListBuilder method is shorter than the accepted shape'
        }
        Assert-Instruction -Instruction $Instructions[0] -Opcode 'new' -CommentPattern 'MetadataObjectFeatureOrderProvider\$ListBuilder'
        Assert-Instruction -Instruction $Instructions[1] -Opcode 'dup'
        Assert-Instruction -Instruction $Instructions[2] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EClass;'
        Assert-Instruction -Instruction $Instructions[3] -Opcode 'aload_0'
        Assert-Instruction -Instruction $Instructions[4] -Opcode 'invokedynamic' -CommentPattern 'InvokeDynamic #41:test:'
        Assert-Instruction -Instruction $Instructions[5] -Opcode 'invokespecial' -CommentPattern 'ListBuilder\."<init>":'
        $pairStart = 6
        $methodClassifier = Get-FieldLiteralToken -Line ([string]$Instructions[2]['comment'])
    } elseif ([string]$Instructions[0]['opcode'] -eq 'aload_0') {
        if ($Instructions.Count -lt 12) {
            throw 'filtered ListBuilder method is shorter than the accepted shape'
        }
        Assert-Instruction -Instruction $Instructions[1] -Opcode 'invokedynamic' -CommentPattern 'InvokeDynamic #42:test:'
        Assert-Instruction -Instruction $Instructions[2] -Opcode 'astore_2'
        Assert-Instruction -Instruction $Instructions[3] -Opcode 'new' -CommentPattern 'MetadataObjectFeatureOrderProvider\$ListBuilder'
        Assert-Instruction -Instruction $Instructions[4] -Opcode 'dup'
        Assert-Instruction -Instruction $Instructions[5] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EClass;'
        Assert-Instruction -Instruction $Instructions[6] -Opcode 'aload_2'
        Assert-Instruction -Instruction $Instructions[7] -Opcode 'invokespecial' -CommentPattern 'ListBuilder\."<init>":'
        $pairStart = 8
        $methodClassifier = Get-FieldLiteralToken -Line ([string]$Instructions[5]['comment'])
    } else {
        throw "method has an unknown ListBuilder prologue: $($Instructions[0]['text'])"
    }
    if ($null -eq $methodClassifier -or [string]$methodClassifier -ne $Classifier) {
        throw "method ListBuilder EClass does not prove classifier '$Classifier'"
    }

    $tailStart = $Instructions.Count - 2
    Assert-Instruction -Instruction $Instructions[$tailStart] -Opcode 'invokevirtual' -CommentPattern 'ListBuilder\.build:'
    Assert-Instruction -Instruction $Instructions[$tailStart + 1] -Opcode 'areturn'
    if (($tailStart - $pairStart) % 2 -ne 0) {
        throw 'ListBuilder operations do not form literal/cursor-or-next pairs'
    }

    $operations = [System.Collections.Generic.List[object]]::new()
    for ($index = $pairStart; $index -lt $tailStart; $index += 2) {
        Assert-Instruction -Instruction $Instructions[$index] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/E(?:StructuralFeature|Reference|Attribute);'
        Assert-Instruction -Instruction $Instructions[$index + 1] -Opcode 'invokevirtual' -CommentPattern 'MetadataObjectFeatureOrderProvider\$ListBuilder\.(cursor|next):'
        $feature = Get-FieldLiteralToken -Line ([string]$Instructions[$index]['comment'])
        if ($null -eq $feature) {
            throw 'ListBuilder operation consumes a nonliteral feature'
        }
        if ([string]$Instructions[$index + 1]['comment'] -notmatch 'ListBuilder\.(cursor|next):') {
            throw 'ListBuilder operation kind is not cursor or next'
        }
        $operations.Add((New-OrderOperation -Operation ([string]$Matches[1]) -Feature $feature))
    }
    if ($operations.Count -eq 0) {
        throw 'method has no literal ListBuilder cursor/next operations'
    }
    return @($operations)
}

function Get-ConfigurationListBuilderVariants {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [object[]]$Instructions,
        [Parameter(Mandatory)] [string]$Classifier
    )

    $branches = @($Instructions | Where-Object {
        [string]$_['opcode'] -match '^(if|goto|tableswitch|lookupswitch|jsr)'
    })
    if ($branches.Count -ne 2 -or
        [string]$branches[0]['opcode'] -ne 'ifeq' -or
        [string]$branches[1]['opcode'] -ne 'goto') {
        throw 'Configuration method does not have the single accepted version ternary.'
    }

    $ifeqIndex = [array]::IndexOf($Instructions, $branches[0])
    $gotoIndex = [array]::IndexOf($Instructions, $branches[1])
    if ($ifeqIndex -lt 3 -or $gotoIndex -ne $ifeqIndex + 2) {
        throw 'Configuration version ternary has an unknown instruction layout.'
    }
    Assert-Instruction -Instruction $Instructions[$ifeqIndex - 3] -Opcode 'aload_1'
    Assert-Instruction -Instruction $Instructions[$ifeqIndex - 2] -Opcode 'getstatic' -CommentPattern 'Version\.V[0-9_]+:'
    Assert-Instruction -Instruction $Instructions[$ifeqIndex - 1] -Opcode 'invokevirtual' -CommentPattern 'Version\.isGreaterThan:'
    Assert-Instruction -Instruction $Instructions[$ifeqIndex + 1] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/E(?:StructuralFeature|Reference|Attribute);'

    if ([string]$branches[0]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$Instructions[$gotoIndex + 1]['offset']) {
        throw 'Configuration false branch target does not select the literal false feature.'
    }
    Assert-Instruction -Instruction $Instructions[$gotoIndex + 1] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/E(?:StructuralFeature|Reference|Attribute);'
    if ([string]$branches[1]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$Instructions[$gotoIndex + 2]['offset']) {
        throw 'Configuration join target does not immediately invoke ListBuilder.'
    }
    Assert-Instruction -Instruction $Instructions[$gotoIndex + 2] -Opcode 'invokevirtual' -CommentPattern 'ListBuilder\.cursor:'

    $version = $null
    if ([string]$Instructions[$ifeqIndex - 2]['comment'] -match 'Version\.(V[0-9_]+):') {
        $version = [string]$Matches[1]
    }
    $trueFeature = Get-FieldLiteralToken -Line ([string]$Instructions[$ifeqIndex + 1]['comment'])
    $falseFeature = Get-FieldLiteralToken -Line ([string]$Instructions[$gotoIndex + 1]['comment'])
    if ($null -eq $version -or $null -eq $trueFeature -or $null -eq $falseFeature) {
        throw 'Configuration version ternary contains a nonliteral operand.'
    }

    # Remove the proven ternary and replace it with each literal alternative,
    # yielding two linear instruction streams that are validated independently.
    $prefix = @($Instructions[0..($ifeqIndex - 4)])
    $suffix = @($Instructions[($gotoIndex + 3)..($Instructions.Count - 1)])
    $variants = [System.Collections.Generic.List[object]]::new()
    foreach ($choice in @(
        [ordered]@{ predicate = "greaterThan($version)"; feature = $trueFeature },
        [ordered]@{ predicate = "notGreaterThan($version)"; feature = $falseFeature }
    )) {
        $featureInstruction = [ordered]@{
            offset = [int]$Instructions[$ifeqIndex + 1]['offset']
            opcode = 'getstatic'
            operand = ''
            comment = "Field Literals.$($choice['feature']):Lorg/eclipse/emf/ecore/EStructuralFeature;"
            text = "synthetic proven branch literal $($choice['feature'])"
        }
        $cursorInstruction = $Instructions[$gotoIndex + 2]
        $linear = @($prefix + @($featureInstruction, $cursorInstruction) + $suffix)
        $variants.Add([ordered]@{
            versionPredicate = [string]$choice['predicate']
            operations = @(Get-LinearListBuilderOperations -Instructions $linear -Classifier $Classifier)
        })
    }
    return @($variants)
}

function New-MetadataPropertyRecord {
    param(
        [Parameter(Mandatory)] $Binding,
        [Parameter(Mandatory)] [string]$VersionPredicate,
        [Parameter(Mandatory)] [AllowEmptyString()] [object[]]$Operations,
        [Parameter(Mandatory)] $FallbackProof
    )

    return [ordered]@{
        provider = 'MetadataObjectFeatureOrderProvider'
        classifier = [string]$Binding['classifier']
        section = 'properties'
        orderedFeatures = @($Operations | ForEach-Object { [string]$_['feature'] })
        orderOperations = @($Operations)
        versionPredicate = $VersionPredicate
        fallback = [string]$FallbackProof['value']
        evidence = [ordered]@{
            status = 'verified'
            kind = 'javap-v-bootstrap-lambda-listbuilder-order'
            sources = @(
                'tools/import-edt-metadata-order.ps1',
                "$providerEvidenceSource#BootstrapMethods"
            )
            note = "constructor invokedynamic CP #$($Binding['constantPoolIndex']) resolves through BootstrapMethods entry $($Binding['bootstrapIndex']) to $($Binding['method']); literal cursor/next bytecode is fail-closed parsed; $($FallbackProof['note'])"
        }
    }
}

function Get-InnerInfoRecords {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines)

    $block = Get-JavapMethodBlock -Lines $Lines -HeaderPattern (
        '^  public java\.util\.List<org\.eclipse\.emf\.ecore\.EStructuralFeature> getInnerInfo\(')
    $instructions = @(ConvertTo-JavapInstructions -Lines $block)
    $records = [System.Collections.Generic.List[object]]::new()
    $index = 0
    while ($index + 1 -lt $instructions.Count -and
        [string]$instructions[$index]['opcode'] -eq 'aload_1' -and
        [string]$instructions[$index + 1]['opcode'] -eq 'getstatic' -and
        [string]$instructions[$index + 1]['comment'] -match 'Lorg/eclipse/emf/ecore/EClass;') {
        if ($index + 5 -ge $instructions.Count) {
            throw 'getInnerInfo special-case chain is truncated.'
        }
        Assert-Instruction -Instruction $instructions[$index + 1] -Opcode 'getstatic' -CommentPattern 'Lorg/eclipse/emf/ecore/EClass;'
        Assert-Instruction -Instruction $instructions[$index + 2] -Opcode 'if_acmpne'
        $classifier = Get-FieldLiteralToken -Line ([string]$instructions[$index + 1]['comment'])
        $features = [System.Collections.Generic.List[string]]::new()
        $cursor = $index + 3
        while ($cursor -lt $instructions.Count -and
            [string]$instructions[$cursor]['opcode'] -eq 'getstatic' -and
            [string]$instructions[$cursor]['comment'] -match 'Lorg/eclipse/emf/ecore/E(?:StructuralFeature|Reference|Attribute);') {
            $feature = Get-FieldLiteralToken -Line ([string]$instructions[$cursor]['comment'])
            if ($null -eq $feature) {
                throw 'getInnerInfo special case contains a nonliteral feature.'
            }
            $features.Add($feature)
            $cursor++
        }
        if ($features.Count -eq 0) {
            break
        }
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'invokestatic' -CommentPattern 'java/util/List\.of:'
        Assert-Instruction -Instruction $instructions[$cursor + 1] -Opcode 'areturn'
        $arityPattern = if ($features.Count -eq 1) {
            'List\.of:\(Ljava/lang/Object;\)Ljava/util/List;'
        } elseif ($features.Count -eq 2) {
            'List\.of:\(Ljava/lang/Object;Ljava/lang/Object;\)Ljava/util/List;'
        } else {
            throw "getInnerInfo special case '$classifier' has an unsupported literal arity."
        }
        Assert-Instruction -Instruction $instructions[$cursor] -Opcode 'invokestatic' -CommentPattern $arityPattern
        $nextOffset = if ($cursor + 2 -lt $instructions.Count) {
            [int]$instructions[$cursor + 2]['offset']
        } else {
            -1
        }
        if ([string]$instructions[$index + 2]['operand'] -notmatch '^(\d+)$' -or
            [int]$Matches[1] -ne $nextOffset) {
            throw "getInnerInfo branch for '$classifier' does not jump to the next proven case."
        }
        $operations = @($features | ForEach-Object {
            New-OrderOperation -Operation 'emit' -Feature $_
        })
        $records.Add([ordered]@{
            provider = 'MetadataObjectFeatureOrderProvider'
            classifier = $classifier
            section = 'internalInfo'
            orderedFeatures = @($features)
            orderOperations = @($operations)
            versionPredicate = 'always'
            fallback = $innerInfoFallback
            evidence = [ordered]@{
                status = 'verified'
                kind = 'javap-v-explicit-inner-info-branch'
                sources = @(
                    'tools/import-edt-metadata-order.ps1',
                    "$providerEvidenceSource#getInnerInfo"
                )
                note = 'literal EClass identity branch returns only the recorded literal feature list'
            }
        })
        $index = $cursor + 2
    }

    $fallback = @($instructions[$index..($instructions.Count - 1)])
    $expectedOpcodes = @(
        'aload_1', 'ldc', 'invokeinterface', 'astore_3', 'aload_3',
        'ifnull', 'aload_3', 'invokestatic', 'goto', 'invokestatic', 'areturn'
    )
    if ($fallback.Count -ne $expectedOpcodes.Count) {
        throw 'getInnerInfo fallback has an unknown instruction count.'
    }
    for ($fallbackIndex = 0; $fallbackIndex -lt $expectedOpcodes.Count; $fallbackIndex++) {
        Assert-Instruction -Instruction $fallback[$fallbackIndex] -Opcode $expectedOpcodes[$fallbackIndex]
    }
    Assert-Instruction -Instruction $fallback[1] -Opcode 'ldc' -CommentPattern 'String producedTypes$'
    Assert-Instruction -Instruction $fallback[2] -Opcode 'invokeinterface' -CommentPattern 'EClass\.getEStructuralFeature:'
    Assert-Instruction -Instruction $fallback[7] -Opcode 'invokestatic' -CommentPattern 'java/util/List\.of:'
    Assert-Instruction -Instruction $fallback[9] -Opcode 'invokestatic' -CommentPattern 'java/util/Collections\.emptyList:'
    if ([string]$fallback[5]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$fallback[9]['offset']) {
        throw 'getInnerInfo null branch does not target Collections.emptyList.'
    }
    if ([string]$fallback[8]['operand'] -notmatch '^(\d+)$' -or
        [int]$Matches[1] -ne [int]$fallback[10]['offset']) {
        throw 'getInnerInfo fallback join does not target the final areturn.'
    }

    if ($records.Count -eq 0) {
        throw 'getInnerInfo contains no verified literal special cases.'
    }
    return @($records)
}

function Get-MetadataProviderRecords {
    param(
        [Parameter(Mandatory)] [AllowEmptyString()] [string[]]$Lines,
        [Parameter(Mandatory)] $FallbackProof
    )

    if (@($Lines).Count -eq 0) {
        throw 'Metadata provider javap output is empty.'
    }
    $constantPoolToBootstrap = Get-InvokeDynamicBootstrapMap -Lines $Lines
    $bootstrapToMethod = Get-LambdaBootstrapTargetMap -Lines $Lines
    $constructorArguments = @{
        Lines = [string[]]$Lines
        ConstantPoolToBootstrap = $constantPoolToBootstrap
        BootstrapToMethod = $bootstrapToMethod
    }
    Write-Verbose "metadata javap lines=$(@($Lines).Count) constructor lines=$(@($constructorArguments['Lines']).Count)"
    $bindings = @(Get-ConstructorBindings @constructorArguments)
    $records = [System.Collections.Generic.List[object]]::new()
    $rejected = [System.Collections.Generic.List[object]]::new()

    foreach ($binding in $bindings) {
        try {
            $method = [string]$binding['method']
            $block = Get-JavapMethodBlock -Lines $Lines -HeaderPattern (
                '^  private java\.util\.List<org\.eclipse\.emf\.ecore\.EStructuralFeature> ' +
                [regex]::Escape($method) + '\(')
            $instructions = @(ConvertTo-JavapInstructions -Lines $block)
            if ($method -eq 'getConfiguration') {
                foreach ($variant in @(Get-ConfigurationListBuilderVariants `
                    -Instructions $instructions -Classifier ([string]$binding['classifier']))) {
                    $records.Add((New-MetadataPropertyRecord -Binding $binding `
                        -VersionPredicate ([string]$variant['versionPredicate']) `
                        -Operations @($variant['operations']) `
                        -FallbackProof $FallbackProof))
                }
            } else {
                $operations = @(Get-LinearListBuilderOperations `
                    -Instructions $instructions -Classifier ([string]$binding['classifier']))
                $records.Add((New-MetadataPropertyRecord -Binding $binding `
                    -VersionPredicate 'always' -Operations $operations `
                    -FallbackProof $FallbackProof))
            }
        }
        catch {
            $rejected.Add([ordered]@{
                provider = 'MetadataObjectFeatureOrderProvider'
                classifier = [string]$binding['classifier']
                method = [string]$binding['method']
                reason = "fail-closed: $($_.Exception.Message)"
            })
        }
    }

    try {
        foreach ($record in @(Get-InnerInfoRecords -Lines $Lines)) {
            $records.Add($record)
        }
    }
    catch {
        $rejected.Add([ordered]@{
            provider = 'MetadataObjectFeatureOrderProvider'
            classifier = $null
            method = 'getInnerInfo'
            reason = "fail-closed: $($_.Exception.Message)"
        })
    }
    return [ordered]@{ records = @($records); rejected = @($rejected) }
}

function ConvertTo-DeterministicJson {
    param([Parameter(Mandatory)] $Value)

    # Compact JSON avoids the different indentation widths emitted by
    # Windows PowerShell 5.1 and PowerShell 7 while preserving insertion order.
    return (($Value | ConvertTo-Json -Depth 16 -Compress).Replace("`r`n", "`n") + "`n")
}

function Write-Utf8LfFile {
    param(
        [Parameter(Mandatory)] [string]$Path,
        [Parameter(Mandatory)] [string]$Text
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $directory = [System.IO.Path]::GetDirectoryName($fullPath)
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    [System.IO.File]::WriteAllText($fullPath, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Get-Sha256Hex {
    param([Parameter(Mandatory)] [string]$Text)

    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Text)
        return ([System.BitConverter]::ToString($sha256.ComputeHash($bytes))).Replace('-', '')
    }
    finally {
        $sha256.Dispose()
    }
}

if (-not (Get-Command javap -ErrorAction SilentlyContinue)) {
    throw 'javap is required for EDT metadata-order research extraction.'
}

$inventoryDocument = Get-Content -LiteralPath $InputInventory -Raw | ConvertFrom-Json
# Windows PowerShell 5.1 can preserve a top-level JSON array as one pipeline
# object. Force one explicit enumeration so both powershell.exe and pwsh see
# the same flat bundle inventory.
$inventory = @($inventoryDocument | ForEach-Object { $_ })
$entries = @($inventory | Where-Object { [string]$_.bundle -eq $bundleName })
if ($entries.Count -ne 1) {
    throw "Input inventory must contain exactly one '$bundleName' entry; found $($entries.Count)."
}

$jar = [string]$entries[0].jar
if ([string]::IsNullOrWhiteSpace($jar) -or -not (Test-Path -LiteralPath $jar -PathType Leaf)) {
    throw "The '$bundleName' inventory entry must provide an existing jar path."
}

$producedLines = @(Invoke-EdtJavap -Jar $jar -ClassName $producedTypesProviderClass)
$producedFallbackProof = Get-ProducedTypesFallbackProof -Lines $producedLines
$produced = Get-ProducedTypesRecords -Lines $producedLines `
    -Fallback ([string]$producedFallbackProof['value'])
$metadataLines = @(Invoke-EdtJavap -Jar $jar -ClassName $providerClass)
$propertiesFallbackProof = Get-PropertiesFallbackProof -Lines $metadataLines
$metadata = Get-MetadataProviderRecords -Lines $metadataLines `
    -FallbackProof $propertiesFallbackProof
$flatProducedRecords = @($produced.records | ForEach-Object { $_ })
$flatProducedRejected = @($produced.rejected | ForEach-Object { $_ })
$flatMetadataRecords = @($metadata.records | ForEach-Object { $_ })
$flatMetadataRejected = @($metadata.rejected | ForEach-Object { $_ })
# Sort by dictionary keys explicitly. Windows PowerShell 5.1 does not resolve
# OrderedDictionary keys through Sort-Object's bare property-name syntax.
$records = @(@($flatProducedRecords) + @($flatMetadataRecords) |
    Sort-Object { $_['provider'] }, { $_['classifier'] }, { $_['section'] },
        { $_['versionPredicate'] })
$rejected = @(@($flatProducedRejected) + @($flatMetadataRejected) |
    Sort-Object { $_['provider'] }, { $_['classifier'] }, { $_['method'] },
        { [string]::Join('|', @($_['orderedFeatures'])) }, { $_['reason'] })

$snapshot = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = '1C:EDT'
        release = $EdtRelease
        derivation = 'derived tokens from javap -v -p -c -constants for the exact metadata XML export bundle; no JAR, bytecode, source, or machine path retained'
    }
    summary = [ordered]@{
        bundle = $bundleName
        verifiedRecords = $records.Count
        rejectedRecords = $rejected.Count
    }
    records = $records
}

$jsonFirst = ConvertTo-DeterministicJson -Value $snapshot
$jsonSecond = ConvertTo-DeterministicJson -Value $snapshot
$shaFirst = Get-Sha256Hex -Text $jsonFirst
$shaSecond = Get-Sha256Hex -Text $jsonSecond
if ($shaFirst -ne $shaSecond) {
    throw 'Determinism check failed: repeated metadata-order generation produced different SHA-256 values.'
}

Write-Utf8LfFile -Path $OutputOrder -Text $jsonFirst

if (-not [string]::IsNullOrWhiteSpace($RejectReport)) {
    $report = [ordered]@{
        schemaVersion = 1
        source = [ordered]@{
            product = '1C:EDT'
            release = $EdtRelease
            derivation = 'fail-closed rejected bytecode patterns; no bytecode, source, or machine path retained'
        }
        summary = [ordered]@{ rejectedRecords = $rejected.Count }
        rejected = $rejected
    }
    Write-Utf8LfFile -Path $RejectReport -Text (ConvertTo-DeterministicJson -Value $report)
}

Write-Output "Wrote $([System.IO.Path]::GetFullPath($OutputOrder))"
Write-Output "verified=$($records.Count) rejected=$($rejected.Count) sha256=$shaFirst"
