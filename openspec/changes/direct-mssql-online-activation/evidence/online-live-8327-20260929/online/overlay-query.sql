SET NOCOUNT ON;
SET STATISTICS IO ON;
SET STATISTICS TIME ON;
-- the shape of dynamic_generation::storage_table_expression + fetch_binary_rows_query for the selected module (2 file names)
SELECT FileName, PartNo, DataSize, BinaryData
FROM (SELECT CASE FileName
        WHEN N'313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f' THEN N'313d9858-3995-4a4c-b2b0-15d2350417b4'
        WHEN N'313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f.0' THEN N'313d9858-3995-4a4c-b2b0-15d2350417b4.0'
        WHEN N'versions_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f' THEN N'versions' ELSE FileName END AS FileName,
             PartNo, DataSize, BinaryData
      FROM [ibcmd_rs_05_online_a1].dbo.[Config]
      WHERE FileName IN (N'313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f', N'313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f.0', N'versions_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f')
         OR (CHARINDEX(N'_dynupdate_', FileName) = 0 AND FileName NOT IN (N'313d9858-3995-4a4c-b2b0-15d2350417b4', N'313d9858-3995-4a4c-b2b0-15d2350417b4.0', N'versions'))) AS storage
WHERE FileName IN (N'313d9858-3995-4a4c-b2b0-15d2350417b4', N'313d9858-3995-4a4c-b2b0-15d2350417b4.0')
ORDER BY FileName, PartNo
OPTION (RECOMPILE);
GO
-- the same read without the overlay
SELECT FileName, PartNo, DataSize, BinaryData FROM [ibcmd_rs_05_online_a1].dbo.[Config]
WHERE FileName IN (N'313d9858-3995-4a4c-b2b0-15d2350417b4', N'313d9858-3995-4a4c-b2b0-15d2350417b4.0') ORDER BY FileName, PartNo OPTION (RECOMPILE);
GO
