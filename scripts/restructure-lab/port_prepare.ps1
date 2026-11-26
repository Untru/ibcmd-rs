# Port acceptance: restore the native and own twins of the cases t1, b1, c1, b2 from their staged backups, then apply the
# native twins natively (under the lab native lock, one apply per hold).
#   pwsh -NoProfile -File port_prepare.ps1 [-Cases t1,b1,c1,b2]
param([string[]]$Cases = @('t1', 'b1', 'c1', 'b2'))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\restructure'
$apply = 'F:\ibcmd\src\ibcmd-rs-04-restructure\scripts\restructure-lab\apply_only.ps1'
$bak = @{
    t1 = "$lab\bak\ibcmd_rs_04_ddl_s1_base_t1_staged.bak"
    b1 = "$lab\bak\ibcmd_rs_04_ddl_s2_b1_base_b1_staged.bak"
    c1 = "$lab\bak\ibcmd_rs_04_ddl_s2_c1_base_c1_staged.bak"
    b2 = "$lab\bak\ibcmd_rs_04_ddl_s2_b2_base_b2_staged.bak"
}
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
foreach ($c in $Cases) {
    foreach ($side in 'nat', 'own') {
        Log "restore p_${c}_$side"
        pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak $bak[$c] -Name "ibcmd_rs_04_ddl_p_${c}_$side" -Track ddl -Purpose "S1 port acceptance, case ${c}: $side twin of the staged state" | Select-Object -Last 1
    }
}
foreach ($c in $Cases) {
    Log "native apply p_${c}_nat"
    pwsh -NoProfile -File $apply -Database "ibcmd_rs_04_ddl_p_${c}_nat" *> "$lab\logs\port_native_$c.log"
    Get-Content "$lab\logs\port_native_$c.log" -Tail 3
}
Log 'done'
