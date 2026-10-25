# Metadata scalar profiles

Pinned research source: [tool1cd MetaTree.txt at 7c4b91c](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt). No code or serialization-tree data is imported.

The serialization sections identify independent fields, including [BusinessProcess](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L55116), [Task](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L55591), [Constant](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L56126), [CommonAttribute](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L56771), [ChartOfCalculationTypes](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L57305), [ExchangePlan](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L57585) and [CalculationRegister](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L58533).

These descriptions establish semantic independence; the following **modern physical positions are independently measured on public platform-built Coverage**, not inferred from the age of the old source. Evidence inputs are pinned in the configuration-set manifests for 8.3.27.2342/XML2.20 and 8.5.1.1150/XML2.21. Compare each CovTemplate with its generated single-property variant. Metadata UUIDs and generated types vary with object identity; only the scalar difference is used to infer a coordinate. This does not establish arbitrary unmeasured profiles.

| Owner / scalar | Owner coordinate | Observation |
| --- | --- | --- |
| CommonAttribute.AutoUse | 6 | stored 0 Use / 1 DontUse |
| CommonAttribute.AuthenticationSeparation | 11 | stored 0 DontUse / 1 Separate |
| CommonAttribute.PasswordMode / MarkNegatives / MultiLine / ExtendedEdit / FillFromFillingValue | nested typed record 2 / 5 / 7 / 17 / 20 | each bool changes independently 0→1 |
| CommonAttribute.FillChecking | nested typed record 13 | FillFromFillingValue at 20 does not change FillChecking |
| Constant history immediate / after-version | 15 / 16 | two independent modern bool tail fields |
| BusinessProcess.CheckUnique / Autonumbering / CreateOnInput | 19 / 20 / 21 | bool properties must not be inferred from the enum carrier |
| BusinessProcess.NumberPeriodicity / CreateTaskInPrivilegedMode | 29 / 31 | a false privilege flag must not refuse a valid periodicity |
| Task.CheckUnique / Autonumbering / EditType | 20 / 21 / 23 | independent bool/bool/enum carriers; EditType 0 InList / 1 InDialog / 2 BothWays |
| Task history immediate / after-version | 50 / 51 | valid true flags were previously rejected as reserved zeroes |
| ExchangePlan history immediate / after-version | 47 / 48 | valid true flags were previously rejected as reserved zeroes |
| ChartOfCalculationTypes.EditType / UseStandardCommands | 24 / 35 | equal default code 1 concealed the reversed property association |
| ChartOfCalculationTypes.QuickChoice / CreateOnInput | 39 / 55 | changing quick choice must not invalidate a separate enum |
| CalculationRegister.UseStandardCommands / IncludeHelpInContents | 24 / 25 | both bool values preserved; empty ChildObjects emitted |

The shared model and legacy reader had duplicated wrong associations for BP/Task/ChartOfCalculationTypes. Both directions are repaired; changing only the decoder would compile the same mistakes back into SQL. Constant and CommonAttribute already had correctly associated shared-model fields; their legacy projection dropped or swapped values.

`tool1cd_metadata_tests` reads actual public CF entries, compares the legacy result to complete independent native XML and compares every compiled owner through the independent legacy reader to the entire native XML, then checks the stored owner AST. Task has the explicit internal-UUID exception below. It also mutates 25 scalar positions to an unsupported code and requires refusal. The existing full Coverage suite remains the broader baseline gate. The former synthetic CCT slot assertions were corrected only where the native single-property evidence disproved them; their other checks are retained.

This report does not claim a SQL apply measurement or infer support for an undocumented storage profile. Final command results and native reruns are recorded in the main issue evidence after execution.

The legacy adapter passes typed flags and bound views to the model-owned XML projection (`metadata_model::legacy_property_xml`); rendering policy does not reside in physical parsing. Scalar reserved-profile validation remains closed and schema-owned. Unknown EditType and FillChecking codes join the rejection checks.

## Task source semantics and internal UUID boundary

Task owner fields 13/14 are validated UUID carriers which native XML does not expose. The old reader already retains them internally; the XML compiler already emits Nil for them. [tool1cd names them mystery properties](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L55643), and some public native Coverage rows are non-Nil. No meaning or regeneration rule is inferred.

The new inverse test therefore validates the native UUIDs and asserts compiled Nil at precisely these two schema-named coordinates, canonicalizes only those carriers for AST comparison, and compares every remaining member exactly. For all seven kinds, compiled row → independent legacy decoder must reproduce the whole native XML. Other kinds retain complete owner-AST equality without exceptions. No previous successful baseline is weakened.

This proves source-semantic conversion for the repaired properties. It does **not** prove raw Task-owner reconstruction, preservation of these opaque carriers or a SQL lossless round trip. Their representation/sidecar and independent native SQL control remain an open follow-up under #570; no application-specific UUID is substituted into the compiler.
