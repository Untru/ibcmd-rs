"""DBSchema (DBSchema.SerializedData == SchemaStorage.CurrentSchema of SchemaID 0)
-> the SQL tables and indexes the platform creates, checked against the real database.

Grammar (1C brace text, UTF-8 BOM):  {0,{<count>,<table>,...}}
 table  = {"<Name>","N",<num>,"", <fields>, <subtables>, <indexes>, 1, "R"|"S", <sepA>, <sepB>, "", 0, 0}
 fields = {<n>, <field>...}      field = {"<name>", <nullable 0|1>, {<n>, <type>...}, "", 0}
 type   = {"B",len,0,"",0}   binary(len) (len bit31 set: varbinary(len&0x7fffffff|max))
          {"L",0,0,"",0}     boolean -> binary(1)
          {"N",p,s,"",0[,1]} numeric(p,s); with the 6th element 1 and s=0 -> int/bigint by p
          {"T",0,0,"",0}     datetime2(0)
          {"S",len,0,"",0}   nvarchar(len&0x7fffffff) if bit31 set else nchar(len); 0 -> max
          {"R",0,0,"<Tbl>"|"",k}  reference (typed: RRef; untyped: TRef+RRef)
          {"V",0,0,"",0}     rowversion (timestamp)
          {"E",0,0,"",0}     composite discriminator (_TYPE binary(1))
 subtable = {"VTnnn","I",0,"<Owner>", <fields>, {0}, <indexes>, 1, "S", {0}, {0}, "", 0, 0}
 index  = {"<name>", <unique>, {<n>, "<field>"...}, <clustered>, <flag>, 0, {0}, 0, 0}
 sepA/sepB = {0} | {1,{{<n>,"<field>"...}}}   data-separator columns (for the table / for its indexes)
"""
import re
import sys

import bracefmt as bf


def s(x):
    return str(x)


def n(x):
    return int(str(x))


class Col:
    def __init__(self, name, typ, nullable):
        self.name, self.typ, self.nullable = name, typ, nullable

    def ddl(self):
        return "%s %s%s" % (self.name, self.typ, "" if self.nullable else " not null")

    def key(self):
        return (self.name, self.typ, self.nullable)


def sql_type(tag, a, b, six):
    """six = the optional 6th element of a numeric type entry: 1 -> int/bigint, 2 -> int identity."""
    if tag == "B":
        if a & 0x80000000:
            ln = a & 0x7FFFFFFF
            return "varbinary(max)" if ln == 0 else "varbinary(%d)" % ln
        return "binary(%d)" % a
    if tag == "L":
        return "binary(1)"
    if tag == "N":
        if six and b == 0:
            return "int" if a <= 10 else "bigint"
        return "numeric(%d, %d)" % (a, b)
    if tag == "T":
        return "datetime2(0)"
    if tag == "S":
        ln = a & 0x7FFFFFFF
        if a & 0x80000000:
            return "nvarchar(max)" if ln == 0 else "nvarchar(%d)" % ln
        return "nchar(%d)" % ln
    if tag == "V":
        return "timestamp"
    raise ValueError(tag)


def field_cols(field):
    fname = s(field[0])
    nullable = n(field[1]) == 1
    types = field[2][1:]
    multi = len(types) > 1
    out = []
    base = "_" + fname
    for ty in types:
        tag = s(ty[0])
        a, b = n(ty[1]), n(ty[2])
        tname = s(ty[3])
        six = len(ty) > 5 and n(ty[5]) in (1, 2)
        if tag in "BLNTSV":
            suffix = {"B": "_B", "L": "_L", "N": "_N", "T": "_T", "S": "_S", "V": ""}[tag] if multi else ""
            out.append(Col(base + suffix, sql_type(tag, a, b, six), nullable))
        elif tag == "E":
            out.append(Col(base + "_TYPE", "binary(1)", nullable))
        elif tag == "R":
            if tname:
                out.append(Col(base + ("_RRRef" if multi else "RRef"), "binary(16)", nullable))
            else:
                out.append(Col(base + ("_RTRef" if multi else "TRef"), "binary(4)", nullable))
                out.append(Col(base + ("_RRRef" if multi else "RRef"), "binary(16)", nullable))
        else:
            raise ValueError("unknown type tag %s in %s" % (tag, fname))
    return out


def sep_fields(sep):
    """[[names]] groups of a sepA/sepB descriptor."""
    if len(sep) < 2:
        return []
    return [[s(x) for x in grp[1:]] for grp in sep[1][1:]] if False else \
        [[s(x) for x in grp[1:]] for grp in sep[1]]


def parse_schema(blob):
    d = bf.parse(blob)
    return d[1][1:]


def table_columns(t, owner=None, parent=None):
    """Columns of a table or subtable entry (owner = owner table entry for subtables)."""
    fields = t[4][1:]
    cols = []
    if owner is not None:
        oname = s(owner[0])
        cols.append(Col("_%s_IDRRef" % oname, "binary(16)", False))
        # separators come from the owner's fields
        ofb = {s(f[0]): f for f in owner[4][1:]}
        for grp in sep_fields(owner[10]):
            for fname in grp:
                cols += field_cols(ofb[fname])
        cols.append(Col("_KeyField", "binary(4)", False))
    for f in fields:
        cols += field_cols(f)
    return cols


def col_names_of_field(fields_by_name, fname):
    return [c.name for c in field_cols(fields_by_name[fname])]


def index_list(t, owner=None):
    """[(sqlname, unique, clustered, [cols])] for a table / subtable, in the platform's creation order."""
    name = s(t[0])
    tname = "_" + (s(owner[0]) + "_" + name if owner is not None else name)
    fields = t[4][1:]
    fb = {s(f[0]): f for f in fields}
    out = []
    if owner is not None:
        ofb = {s(f[0]): f for f in owner[4][1:]}
        seps = []
        for grp in sep_fields(owner[10]):
            for fname in grp:
                seps += col_names_of_field(ofb, fname)
        own_id = "_%s_IDRRef" % s(owner[0])
        for i, ix in enumerate(t[6][1:], 1):
            cols = list(seps)
            for fn in ix[2][1:]:
                fn = s(fn)
                if fn == "ID":
                    cols.append(own_id)
                else:
                    cols += col_names_of_field(fb, fn)
            out.append(_ix("%s_%d" % (tname, i), n(ix[1]) == 1, n(ix[3]) == 1, cols))
        out.append(("%s_SK" % tname, True, True, seps + [own_id, "_KeyField"]))
        return out
    seps_b = []
    for grp in sep_fields(t[9]):
        for fname in grp:
            seps_b += col_names_of_field(fb, fname)
    for i, ix in enumerate(t[6][1:], 1):
        cols = list(seps_b)
        for fn in ix[2][1:]:
            cols += col_names_of_field(fb, s(fn))
        out.append(_ix("%s_%d" % (tname, i), n(ix[1]) == 1, n(ix[3]) == 1, cols))
    return out


def implicit_indexes(t):
    """Indexes the platform adds to an object's main table beyond the DBSchema index list.
    -> [(name or None for an inline auto-named PK, unique, clustered, cols)]"""
    name = s(t[0])
    idf = [f for f in t[4][1:] if s(f[0]) == "ID"]
    if not idf:
        return []
    ty = idf[0][2][1]
    if not (s(ty[0]) == "R" and s(ty[3]) == name and n(ty[4]) == 2):
        return []
    fb = {s(f[0]): f for f in t[4][1:]}
    seps_a = []
    for grp in sep_fields(t[9]):
        for fname in grp:
            seps_a += col_names_of_field(fb, fname)
    out = []
    if seps_a:
        out.append(("_%s_S_HPK" % name, True, True, seps_a + ["_IDRRef"]))
        if not sep_fields(t[10]):
            out.append(("_%s_S_PK" % name, True, False, ["_IDRRef"]))
    else:
        out.append((None, True, True, ["_IDRRef"]))
    return out


def _ix(name, unique, clustered, cols):
    """SQL Server allows 16 key columns: a longer index is cut and stops being unique."""
    if len(cols) > 16:
        return (name, False, clustered, cols[:16])
    return (name, unique, clustered, cols)


def create_table_sql(name, cols, inline_pk=None):
    body = ",\n".join(c.ddl() + (" primary key" if inline_pk == c.name else "") for c in cols)
    return "create table dbo.%s (\n%s\n)\n;alter table dbo.%s SET (LOCK_ESCALATION = DISABLE);" % (name, body, name)


def create_index_sql(tname, iname, unique, clustered, cols, ng=False):
    sfx = "NG" if ng else ""
    return "CREATE %s%sINDEX %s%s ON dbo.%s%s (%s) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)%s" % (
        "UNIQUE " if unique else "", "CLUSTERED " if clustered else "", iname, sfx, tname, sfx, ", ".join(cols),
        "" if clustered else " ON [PRIMARY]\n")


if __name__ == "__main__":
    pass
