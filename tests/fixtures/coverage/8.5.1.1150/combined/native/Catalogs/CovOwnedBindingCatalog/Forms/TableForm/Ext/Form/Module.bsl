&AtClient
Procedure ProbeHandler(Command)
EndProcedure

&AtClient
Procedure ProbeBeforeWrite(Cancel, WriteParameters)
EndProcedure

&AtServer
Procedure ProbeBeforeWriteAtServer(Cancel, CurrentObject, WriteParameters)
EndProcedure
