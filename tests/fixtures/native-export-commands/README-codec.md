# Public codec captures

Only the six XML files required by ibcmd-xml/tests/config_dump_info.rs are ported
from #362 commit 30b0461cf1e7b2291577aa9748e88f2d5e87ba43. They are fresh native
8.5.1.1592 captures of the public tiny filedb/common-module fixture (2.20/2.21
empty inventory and change/add/delete/rename ConfigDumpInfo). No ERP/private
configuration files or native CLI implementation are included here.

These compare canonical native BOM/CRLF/attribute spelling. They do not imply
lossless preservation of arbitrary comments or formatting in input XML.
