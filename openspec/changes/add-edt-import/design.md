# Design

Use explicit selection so ordinary XML imports keep their existing syntax. The profile selects the EDT reader and associated XML dialect independently of database credentials or DBMS. The source reader must finish file accounting before any backend writes. Extension imports must retain their explicit target binding.

Both existing EDT adapter entry points render Designer XML to a directory internally. Returning a SourceTree from that directory still incurs the intermediate write. The eventual staging interface must consume validated descriptor and asset bytes, including the source used by the verification guard. File and PostgreSQL share a stage planner. SQL Server has separate patch and base-free planners. Extension staging currently binds to SQL Server.
