"""Compare ConfigDumpInfo byte-for-byte, ignoring only configVersion attributes."""
import json
import re
import sys
from pathlib import Path


def normalized(path):
    return re.sub(rb'configVersion="[^"]*"', b'configVersion=""', Path(path).read_bytes())


if __name__ == "__main__":
    equal = normalized(sys.argv[1]) == normalized(sys.argv[2])
    print(json.dumps({"equal_except_config_version": equal}))
    sys.exit(0 if equal else 1)
