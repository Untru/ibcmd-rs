$ErrorActionPreference='Stop'
# Extract the actual copied bounded implementation; omit runtime wrappers only.
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile("$PSScriptRoot/compact8-tools/live/process.ps1",[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'copied helper parser'}
foreach($name in @('New-LiveBoundedProcess','Invoke-LiveBounded')){
 $node=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$false))
 if($node.Count -ne 1){throw 'actual helper function missing'}
 . ([scriptblock]::Create($node[0].Extent.Text))
}
function Assert-Compact8RuntimeClosure {} # No FIFO/DB/lifecycle; only tiny Python and its tiny child.
function Get-LiveChildReceiptRoot {return $null}
function Save-LiveUncertainChild {throw 'unexpected uncertainty in tiny stdin test'}
$script:LiveBoundedUncertainChild=$false
$code='import sys,subprocess; assert sys.stdin.read()==""; r=subprocess.run([sys.executable,"-c","print(12345)"],check=True,capture_output=True,text=True); assert r.stdout.strip()=="12345"; print("EOF-and-nested-stdin-PASS")'
$result=Invoke-LiveBounded 'C:/Program Files/Python313/python.exe' @('-c',$code) 30
if($result.ExitCode -ne 0 -or $result.Stderr -or $result.Stdout.Trim() -cne 'EOF-and-nested-stdin-PASS' -or $script:LiveBoundedUncertainChild){throw 'actual nested Python stdin proof failed'}
'PASS actual copied helper: valid redirected stdin/EOF + nested tiny Python; SQL/native/FIFO/registry/signals/deletes0'
