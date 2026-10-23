import re
import shutil
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
kinds = set(sys.argv[3].split(","))

FOLDERS = {
    "Language": "Languages",
    "Subsystem": "Subsystems",
    "Style": "Styles",
    "CommonPicture": "CommonPictures",
    "CommonModule": "CommonModules",
    "CommonForm": "CommonForms",
    "CommonCommand": "CommonCommands",
    "CommandGroup": "CommandGroups",
    "CommonTemplate": "CommonTemplates",
    "Constant": "Constants",
    "Catalog": "Catalogs",
    "Document": "Documents",
    "Enum": "Enums",
    "Report": "Reports",
    "DataProcessor": "DataProcessors",
    "ExchangePlan": "ExchangePlans",
    "ExternalDataSource": "ExternalDataSources",
    "InformationRegister": "InformationRegisters",
    "AccumulationRegister": "AccumulationRegisters",
    "ChartOfCharacteristicTypes": "ChartsOfCharacteristicTypes",
    "FilterCriterion": "FilterCriteria",
    "FunctionalOption": "FunctionalOptions",
    "HTTPService": "HTTPServices",
    "XDTOPackage": "XDTOPackages",
    "ScheduledJob": "ScheduledJobs",
    "SessionParameter": "SessionParameters",
    "SettingsStorage": "SettingsStorages",
    "EventSubscription": "EventSubscriptions",
    "Role": "Roles",
    "Bot": "Bots",
}

if dst.exists():
    shutil.rmtree(dst)
shutil.copytree(src, dst)

dump = dst / "ConfigDumpInfo.xml"
if dump.exists():
    dump.unlink()

config_path = dst / "Configuration.xml"
text = config_path.read_text(encoding="utf-8-sig")
start = text.index("<ChildObjects>")
end = text.index("</ChildObjects>")
block = text[start:end]
entry = re.compile(r"[ \t]*<(?P<kind>\w+)>(?P<name>[^<]+)</(?P=kind)>\r?\n")
removed = []


def drop(match: re.Match[str]) -> str:
    kind = match.group("kind")
    if kind not in kinds:
        return match.group(0)
    removed.append((kind, match.group("name")))
    return ""


new_block = entry.sub(drop, block)
text = text[:start] + new_block + text[end:]
config_path.write_text(text, encoding="utf-8")

for kind, name in removed:
    folder_name = FOLDERS.get(kind)
    if folder_name is None:
        raise SystemExit(f"unknown folder for {kind}")
    folder = dst / folder_name
    candidate = folder / f"{name}.xml"
    if candidate.exists():
        candidate.unlink()
    nested = folder / name
    if nested.is_dir():
        shutil.rmtree(nested)

references = {f"{kind}.{name}" for kind, name in removed}

# Command-interface assets list objects by name inside their own structure;
# dropping single lines would unbalance the XML. They are optional, so remove
# the files whole.
for asset in (dst / "Ext").glob("*.xml") if (dst / "Ext").is_dir() else []:
    if "CommandInterface" in asset.name:
        asset.unlink()

for path in dst.rglob("*.xml"):
    if path.name in ("Configuration.xml", "ConfigDumpInfo.xml"):
        continue
    if "CommandInterface" in path.name:
        continue
    original = path.read_text(encoding="utf-8-sig")
    kept = [
        line
        for line in original.splitlines(keepends=True)
        if not any(reference in line for reference in references)
    ]
    if len(kept) != len(original.splitlines(keepends=True)):
        path.write_text("".join(kept), encoding="utf-8")

# The configuration's own properties may name a removed object
# (`<DefaultStyle>Style.Стиль1</DefaultStyle>`). Empty such an element instead
# of deleting the line: the property still has to exist.
element = re.compile(r"^(?P<indent>\s*)<(?P<tag>[\w:]+)>\s*(?P<value>[^<]*?)\s*</(?P=tag)>\s*$")
item = re.compile(r"^\s*<xr:Item\b[^>]*>[^<]*</xr:Item>\s*$")
lines = config_path.read_text(encoding="utf-8").splitlines(keepends=True)
rewritten = []
changed = 0
dropped = 0
for line in lines:
    match = element.match(line)
    if match and match.group("value") in references:
        rewritten.append(f"{match.group('indent')}<{match.group('tag')}/>\n")
        changed += 1
        continue
    if item.match(line) and any(reference in line for reference in references):
        dropped += 1
        continue
    rewritten.append(line)
config_path.write_text("".join(rewritten), encoding="utf-8")

print("removed", len(removed), "entries;", changed, "properties emptied;", dropped, "list items dropped")

# Usage:
#   python tools/rea/prune_export.py <src-tree> <dst-tree> "Kind1,Kind2"
#
# Removes the listed metadata kinds from a native XML export so the
# XML -> infobase -> apply route can be bisected: the Configuration.xml child
# list, the kind's own files/folders and every line that names a removed
# object are dropped. ConfigDumpInfo.xml is deleted (editing it corrupts it).
#
# Known limitation: references held in Configuration.xml properties (for
# example DefaultStyle) are not rewritten yet, so pruning a referenced kind
# stops import with "Неизвестный объект метаданных". That cleanup is the next
# step in the import investigation.

# Bisect status (Round 15):
#   - minimal tree (Language only) imports AND applies: the XML route works
#     once references are cleaned;
#   - pruning a whole kind still leaves typed references in other objects
#     ("Неизвестное имя типа - ChartOfCharacteristicTypesRef.X"), so the next
#     step is dependency-aware pruning: remove the referrers (attributes,
#     resources, subsystem items) together with the kind, or prune objects
#     inside a kind instead of kinds.
