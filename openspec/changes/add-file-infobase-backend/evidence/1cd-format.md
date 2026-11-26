# 1Cv8.1CD, format 8.3.8: what the configuration tables need

Measured on 21.10.2026 on files written by 1C:Enterprise 8.5.1.1150 (`ibcmd`,
Linux x86_64): an empty infobase (`infobase create --create-database`), the
БСП 3.2 demo configuration (`infobase create --load=1Cv8_demo.cf --apply`),
the same after a native `config import` of its own export (a staged
`CONFIGSAVE`), and an empty infobase after a native `config import` of the
БСП tree. The small files of `tests/fixtures/file-infobase` are from the same
run. Every layout below was checked by reading those files and, for the
writer, by `config apply` and `config export` of the platform on files
`ibcmd-filedb` wrote (see `bsp-8-5-roundtrip-20261021.md`).

All integers are little-endian unless said otherwise.

## Pages

Page 0, the file header:

| offset | size | value |
|---|---|---|
| 0 | 8 | `1CDBMSV8` |
| 8 | 4 | format `8 3 8 0` |
| 12 | 4 | pages in the file |
| 16 | 4 | 1 in every file seen |
| 20 | 4 | page size: 8192 in every file seen (`ibcmd` default) |

The file is exactly `pages × page size` bytes. Page 1 is the free-pages object
(signature `0xFF1C`; empty in every file seen, the platform keeps freed space
inside the objects, see below). Page 2 is the root object.

## Objects

Every table part and the root are objects: a header page and data pages.

| offset | size | value |
|---|---|---|
| 0 | 2 | `0xFD1C` |
| 2 | 2 | fat level: 0 — the list holds data pages; 1 — it holds pages of `page_size / 4` data page numbers |
| 4 | 4 | change counter |
| 8 | 4 | length-change counter |
| 12 | 4 | number of second-level pages (fat level 1), else 0 |
| 16 | 8 | object length in bytes |
| 24 | … | `(page_size − 24) / 4` page numbers (2042 for 8 KB pages) |

The БСП `CONFIG` blob (103 MB, 12 578 pages) has fat level 1 and 7
second-level pages. An object crosses page boundaries freely: records and
blob blocks are offsets into the object, not into pages.

## Blobs

A blob object is 256-byte blocks: `u32` next block, `u16` used bytes, 250 data
bytes. Block 0 is the header; its first `u32` heads the chain of free blocks,
linked through their first `u32`. A value is the chain from its first block to
a block whose next is 0. The root object is a blob: its chain at block 1
holds a 32-byte locale (`en_US_POSIX`), a `u32` table count and one `u32` per
table — the first block of that table's description.

## Table descriptions

UTF-8 text in the platform's brace format. The configuration tables
(`CONFIG`, `CONFIGSAVE`, `PARAMS`, `FILES`, `CONFIGCAS`, `CONFIGCASSAVE`) all
read:

```text
{"CONFIG",0,
{"Fields",
{"FILENAME","NVC",0,128,0,"CI"},
{"CREATION","DT",0,0,0,"CS"},
{"MODIFIED","DT",0,0,0,"CS"},
{"ATTRIBUTES","N",0,5,0,"CS"},
{"DATASIZE","N",0,20,0,"CS"},
{"BINARYDATA","I",0,0,0,"CS"},
{"PARTNO","N",0,10,0,"CS"}
},
{"Indexes",
{"BYNAMENO_CONFIG",0,
{"FILENAME",128},
{"PARTNO",0}
}
},
{"Recordlock","0"},
{"Files",6,7,8}
}
```

`Files` names the header pages of the record, blob and index objects.

## Records

A record is a free flag byte and the fields, 301 bytes for the configuration
tables:

| field | bytes | encoding |
|---|---|---|
| `FILENAME` NVC(128) | 258 | `u16` character count, UTF-16LE, **padded with spaces** (`20 00`) |
| `CREATION`, `MODIFIED` DT | 7 each | packed decimal `yyyyMMddhhmmss` |
| `ATTRIBUTES` N(5) | 3 | packed decimal: sign nibble 1, digits, pad nibble |
| `DATASIZE` N(20) | 11 | packed decimal; the size of the whole row in every part |
| `BINARYDATA` I | 8 | `u32` first blob block, `u32` length |
| `PARTNO` N(10) | 6 | packed decimal |

Record 0 is a record of field defaults (an empty name of spaces, zero dates,
packed zeros) with the free flag 1 and the number of the first free record
written over its start; a free record is the same flag and the next free
record number (0 ends the chain). After `config apply` every `CONFIGSAVE`
record and blob block is on these chains — the object keeps its pages, and
the next import reuses them (the native import into the БСП file grew it by
5 pages for 170 MB of rows).

Rows are stored raw-deflated, as in SQL Server. A row longer than
10 000 000 bytes is stored as parts 0, 1, … of 10 000 000 bytes; each part
carries the whole `DATASIZE`. `ATTRIBUTES` is 0 in every row seen. A staged
`CONFIGSAVE` holds every row of the configuration, `root`, `version`,
`versions` and a `deleted` row (`﻿0` when nothing is removed); an import
into an empty infobase stages `*.new` rows, which the export, as for SQL
Server, does not publish.

## The index

Page 0 of the index object: `u32` head of the free page chain, then one `u32`
per index — the page of its descriptor: `u32` root page, `u16` key length
(392 for the configuration tables), `u16` `0x0080`. Tree pages start with
`u16` flags (1 root, 2 leaf, 3 both, 0 inner), `u16` entry count, `u32`
previous and `u32` next page on the same level (`0xFFFFFFFF` at the ends).

A branch entry is the key of the last entry under the child, then that
entry's record number and the child page, both **big-endian** (checked on all
154 branch entries of the БСП `CONFIG` index).

A leaf adds `u16` free bytes, `u32` record mask, `u16` left and right masks,
`u16` record, left and right bit widths and `u16` bytes per entry — `ffffffff
ffff ffff 32 16 16 8` for the configuration tables. Entries grow from the
header, key bytes from the end of the page; an entry packs the record number,
`left` (bytes shared with the previous key) and `right` (trailing zero bytes),
and the page stores the middle. Free bytes = page − 30 − entries × 8 − stored
key bytes (all 117 leaves of a native `CONFIGSAVE`).

Two rules matter to the platform when it changes the tree, and break it (a
heap corruption in `config apply`) when they are not kept:

- `left` never reaches into the trailing zeros of the previous key: native
  leaves store part 1 of a row after part 0 with `left` 93 where the keys
  share 97 bytes. The platform rebuilds keys in a buffer holding only the
  significant part.
- Leaves and branches are about half full (native leaves: 60–93 entries,
  3.5–5 KB free; branches 11 of 20 entries). Removing an entry makes the next
  one store its shared prefix again, so a full leaf overflows when the
  platform empties `CONFIGSAVE`.

### Keys

The key of a configuration-table row is an ICU-style sort key of `FILENAME`
under the infobase collation `en_US_POSIX` at secondary strength, `00`, the
packed decimal of `PARTNO`, zero padded to 392 bytes:

- each character: `28` then `c − 0x1C` for `0x21…0x5A` (digits `14…1D`, `-`
  `11`, `.` `12`, `A…Z` `25…3E`), or `28 3F` then `c − 0x57` for `0x5B…0x7E`
  (`_` `08`, `a…z` `0A…23`) — code-point order, upper case before lower case,
  as the POSIX tailoring sorts;
- `01`, then the secondary weights compressed to one byte `0x04 + length`;
- `00`, then `PARTNO`.

The formula reproduces every key of every native index read — 9 938
(`CONFIG`), 9 939 (`CONFIGSAVE`), 9 934 (the `*.new` stage), 25 (`PARAMS`), 7
(`FILES`) — byte for byte, record numbers included. It is evidenced for the
characters `0-9 A-Z a-z - . / _` and names of 1 to 43 characters;
`ibcmd-filedb` refuses any other name rather than guess its key.

### The collation is the infobase's locale

`ibcmd infobase create` takes the locale of the process unless `--locale`
says otherwise, so a Russian platform creates `ru_RU` infobases; the root
object holds the locale and the keys follow it. Under `ru_RU` (infobases
created with `--locale=ru_RU`: empty, the БСП demo, a native import of the
БСП tree into each):

- a digit is `0x12 + 2d`, a Latin letter `0x27 + 2(l − 'a')` whatever its
  case (`CAS_GC_Info` and `cas_gc_info` have one key), `-` is `06 10`, `_`
  `06 0C`, `.` `09`, `/` `0B 67`; no byte separates characters;
- the secondary byte and `PARTNO` are as under `en_US_POSIX`.

This reproduces all 29 855 keys of the `CONFIG`, `CONFIGSAVE`, `PARAMS` and
`FILES` indexes of those infobases. A `de_DE` infobase (`--locale=de_DE`) has
keys neither formula gives. `ibcmd-filedb` therefore writes only `en_US_POSIX`
and `ru_RU` infobases, and before a write checks every configuration-table
index of the infobase — keys the platform wrote — against its formula; any
other locale, or a mismatch, refuses the write. Reading does not use keys and
works for any locale.

## Locking

While an infobase is open the platform (8.5.1.1150, Linux) holds a POSIX read
lock on one byte of `1Cv8.1CD` at offset `i64::MAX − 1`, and keeps `1Cv8.1CL`
and `*.cfl` files beside it. A POSIX write lock on the whole file by another
process makes the platform refuse to open the infobase: `Database shared
access error`. `ibcmd-filedb` takes that write lock for an import (so an open
infobase is refused, and the platform cannot open one being written) and a
read lock for an export (compatible with an idle open infobase).
