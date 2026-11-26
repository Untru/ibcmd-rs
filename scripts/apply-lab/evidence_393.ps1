# Collects the checks of the #393 twins into one text file (evidence for docs/apply/evidence/own-apply/).
#   pwsh -NoProfile -File evidence_393.ps1 -Out <file>
param([Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONIOENCODING = 'utf-8'
$env:DDL_LAB = 'F:\ibcmd\lab\04\apply\ddlkit'
$lab = 'F:\ibcmd\lab\04\apply'
$kit = (Resolve-Path (Join-Path $PSScriptRoot '..\restructure-lab')).Path
$lines = New-Object System.Collections.Generic.List[string]
function Add($text) { foreach ($l in @($text)) { $lines.Add([string]$l) } }
function Section($title) { Add ''; Add ('=' * 100); Add $title; Add ('=' * 100) }
function Trim-Diff($text) {
    # drop the long listings of added/removed rows of the caches and the help index, keep the summaries
    $text | Where-Object { $_ -notmatch '^    [mM] ' -and $_ -notmatch '^    [-+] [0-9a-f]{40} ' -and $_ -notmatch '^    [-+] user(Docs|Postings|Vocabulary)' }
}

Section 'Twin 1: the form ВсеЗаметки and the template ДатыПасха removed. Stage: this repository''s import (import-override build) of tree rem2. fdp = native apply, fdo = this apply, same stage.'
Add ("Databases: ibcmd_rs_04_apply_fdp_20260930 (native), ibcmd_rs_04_apply_fdo_20260930 (ours); staged backup bak\fd_ours_staged.bak")
Section 'deleted_cmp.py: the platform''s list (two runs, fdk and fdd) against the import''s (snapshot of the twin''s staged state)'
Add (& python "$PSScriptRoot\deleted_cmp.py" "$lab\work\fd\native_deleted.txt" "$lab\work\fd\native_deleted_fdd.txt" 'snap:ibcmd_rs_04_apply_fdp_20260930/staged' 2>&1)
Push-Location $kit
Section 'snapdiff.py fdp/nat_after -> fdo/own_after (listings of cache rows and help-index chunks left out)'
Add (Trim-Diff (& python snapdiff.py ibcmd_rs_04_apply_fdp_20260930 nat_after ibcmd_rs_04_apply_fdo_20260930 own_after --max 8 2>&1))
Section 'si_diff.py: the .si rows compared after inflate'
Add (& python si_diff.py ibcmd_rs_04_apply_fdp_20260930 nat_after ibcmd_rs_04_apply_fdo_20260930 own_after --max-lines 6 2>&1 | ForEach-Object { if ($_.Length -gt 240) { $_.Substring(0, 240) + ' ...' } else { $_ } })
Pop-Location
Section 'reg_cmp.py fdp fdo (change registers)'
Add (& python "$PSScriptRoot\reg_cmp.py" ibcmd_rs_04_apply_fdp_20260930 ibcmd_rs_04_apply_fdo_20260930 --show 2 2>&1 | ForEach-Object { if ($_.Length -gt 300) { $_.Substring(0, 300) + ' ...' } else { $_ } })
Section 'Control twin (native import of the unedited tree, native apply: fdc) against the removal twin (native, fdk): the removal''s effect on the register'
Add (& python "$PSScriptRoot\reg_cmp.py" ibcmd_rs_04_apply_fdc_20260930 ibcmd_rs_04_apply_fdk_20260930 --show 2 2>&1)
Section 'Native export of our result against the tree rem2 (ibcmd-rs source-diff)'
$d = Get-Content "$lab\out\export\fdo.diff.json" -Raw -Encoding UTF8 | ConvertFrom-Json
Add ("summary: left_only {0}, right_only {1}, different {2}, unchanged {3}; differing files: {4}" -f $d.summary.left_only, $d.summary.right_only, $d.summary.different, $d.summary.unchanged, (($d.differences | Where-Object { $_.status -ne 'unchanged' } | ForEach-Object { $_.path }) -join ', '))
Section 'A repeated native config apply on our result'
Add (Get-Content "$lab\logs\native_fdo_apply2.out" -Encoding UTF8)

Section 'Twin 2: the same and an attribute removed (Catalog _ДемоГруппыДоступаНоменклатуры), gate S1. mxp = native apply, mxo = this apply (--allow-restructure s1), same stage.'
Push-Location $kit
Add (Trim-Diff (& python snapdiff.py ibcmd_rs_04_apply_mxp_20260930 nat_after ibcmd_rs_04_apply_mxo_20260930 own_after --max 8 2>&1))
Add (& python si_diff.py ibcmd_rs_04_apply_mxp_20260930 nat_after ibcmd_rs_04_apply_mxo_20260930 own_after --max-lines 4 2>&1 | ForEach-Object { if ($_.Length -gt 240) { $_.Substring(0, 240) + ' ...' } else { $_ } })
Add (& python dbschema_cmp.py ibcmd_rs_04_apply_mxp_20260930 nat_after ibcmd_rs_04_apply_mxo_20260930 own_after 2>&1 | ForEach-Object { if ($_.Length -gt 300) { $_.Substring(0, 300) + ' ...' } else { $_ } })
Add (& pwsh -NoProfile -File compare_tables.ps1 -A ibcmd_rs_04_apply_mxp_20260930 -B ibcmd_rs_04_apply_mxo_20260930 -Tables _Reference3347 2>&1)
Pop-Location
Add (& python "$PSScriptRoot\reg_cmp.py" ibcmd_rs_04_apply_mxp_20260930 ibcmd_rs_04_apply_mxo_20260930 --show 1 2>&1 | ForEach-Object { if ($_.Length -gt 300) { $_.Substring(0, 300) + ' ...' } else { $_ } })
$d = Get-Content "$lab\out\export\mxo.diff.json" -Raw -Encoding UTF8 | ConvertFrom-Json
Add ("native export of our result against the tree mix: left_only {0}, right_only {1}, different {2}, unchanged {3}; differing files: {4}" -f $d.summary.left_only, $d.summary.right_only, $d.summary.different, $d.summary.unchanged, (($d.differences | Where-Object { $_.status -ne 'unchanged' } | ForEach-Object { $_.path }) -join ', '))
Add 'native config apply on our result:'
Add (Get-Content "$lab\logs\native_mxo_apply2.out" -Encoding UTF8)
Add 'native config check on our result:'
Add (Get-Content "$lab\logs\native_mxo_check.out" -Encoding UTF8)

Section 'Twin 3: the register of the removed objects with message numbers and a missing row. fdq = native apply, fdq2 = this apply; the same stage and register state'
Add 'before (reg_state.py show on the state set up by reg_state.py setup; node prefixes, _MessageNo, file rows):'
Add "  form     [('0190A866', 7, 2), ('0190A866', None, 2), ('84D4816E', None, 2), ('B9E29B2B', 0, 2)]   (no row at 8792107B)"
Add "  template [('0190A866', 7, 1), ('0190A866', None, 1)]                                              (no row at 8792107B)"
Add 'after the native apply (fdq):'
Add (& python "$PSScriptRoot\reg_state.py" ibcmd_rs_04_apply_fdq_20260930 show 2>&1)
Add 'after this apply (fdq2):'
Add (& python "$PSScriptRoot\reg_state.py" ibcmd_rs_04_apply_fdq2_20260930 show 2>&1)
Add (& python "$PSScriptRoot\reg_cmp.py" ibcmd_rs_04_apply_fdq_20260930 ibcmd_rs_04_apply_fdq2_20260930 --show 0 2>&1 | Select-Object -First 4)

Section 'Rehearsal (fdr): snapdiff before / after'
Push-Location $kit
Add (& python snapdiff.py ibcmd_rs_04_apply_fdr_20260930 before ibcmd_rs_04_apply_fdr_20260930 after_rehearsal 2>&1 | Select-Object -First 14)
Pop-Location
Section 'Lists refused whole (tools\refusal_cases.ps1)'
Add (Get-Content "$lab\out\refusal_cases.txt" -Encoding UTF8 | Where-Object { $_ -notmatch '^\s+"reason"' } | ForEach-Object { if ($_.Length -gt 420) { $_.Substring(0, 420) + ' ...' } else { $_ } })

[IO.File]::WriteAllLines($Out, $lines, (New-Object System.Text.UTF8Encoding($false)))
"written $Out ($($lines.Count) lines)"
