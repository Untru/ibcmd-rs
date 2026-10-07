param([string]$Database='ibcmd_rs_05_load_w3_work_20261001')
[Console]::OutputEncoding=[Text.Encoding]::UTF8
$ErrorActionPreference='Stop'
if($Database -ne 'ibcmd_rs_05_load_w3_work_20261001'){throw 'exact owned database only'}
$flags=[Reflection.BindingFlags]
function Get-Com($object,$name){[System.__ComObject].InvokeMember($name,$flags::GetProperty,$null,$object,$null)}
function Call-Com($object,$name,$args1){[System.__ComObject].InvokeMember($name,$flags::InvokeMethod,$null,$object,$args1)}
$connector=New-Object -ComObject V83.COMConnector
$base=$connector.Connect('Srvr="localhost:2541";Ref="'+$Database+'";Usr="Администратор";Pwd="";')
try{
$q=$base.NewObject('Запрос');$q.Text="ВЫБРАТЬ Ссылка КАК DocRef, Комментарий КАК Mark, Проведен КАК Posted ИЗ Документ._ДемоСчетНаОплатуПокупателю ГДЕ Комментарий ПОДОБНО &Pattern"
[void](Call-Com $q 'УстановитьПараметр' @('Pattern','ibcmd-rs-load:base%'))
$result=Call-Com $q 'Выполнить' $null; $s=Call-Com $result 'Выбрать' $null
while((Call-Com $s 'Следующий' $null)){$ref=Get-Com $s 'DocRef';$id=Call-Com $ref 'УникальныйИдентификатор' $null;$idText=Call-Com $base 'XMLСтрока' @($id);$mark=Get-Com $s 'Mark';$posted=Get-Com $s 'Posted';Write-Output ($mark+'|'+$idText+'|'+$posted)}
}finally{[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($base);[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($connector)}
