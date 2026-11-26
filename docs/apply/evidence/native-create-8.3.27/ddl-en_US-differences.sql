-- Statements of the en_US create that the ru_RU create does not run (whitespace-normalized), in order.
-- Statements of the ru_RU run absent from the en_US run: 0 (the same tables are created; only their column
-- collation text differs).

ALTER DATABASE ibcmd_rs_04_trace_c3_new3 COLLATE Latin1_General_CI_AS
GO

alter table [Config] drop [ByNameNo_Config] alter table [ConfigCAS] drop [ByNameNo_ConfigCAS] alter table [ConfigCASSave] drop [ByNameNo_ConfigCASSave] alter table [ConfigSave] drop [ByNameNo_ConfigSave] alter table [DepotFiles] drop [ByNameNo_DepotFiles] alter table [Files] drop [ByNameNo_Files] alter table [Params] drop [ByNameNo_Params] alter table [Config] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [ConfigCAS] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [ConfigCASSave] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [ConfigSave] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [DepotFiles] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [Files] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [Params] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null alter table [Config] add primary key ([FileName],[PartNo]) alter table [ConfigCAS] add primary key ([FileName],[PartNo]) alter table [ConfigCASSave] add primary key ([FileName],[PartNo]) alter table [ConfigSave] add primary key ([FileName],[PartNo]) alter table [DepotFiles] add primary key ([FileName],[PartNo]) alter table [Files] add primary key ([FileName],[PartNo]) alter table [Params] add primary key ([FileName],[PartNo])
GO

