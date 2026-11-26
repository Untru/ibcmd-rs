"""Where the scripts find what lies outside this repository. Each is an
environment variable; the default sits next to the checkout.

    ONECDEC_TOOLS   onecdec's platform runner (v8dump.py):  ../1c-tools/onecdec
    IBCMD_EXE       the ibcmd-rs build:                     target/release/ibcmd-rs(.exe)
    IBCMD_WORK      scratch and native dumps of the corpora: ../ibcmd-rs-work
    IBCMD_CORPUS    the external-object corpus:             ../ibcmd-rs-corpus
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BESIDE = os.path.dirname(REPO)

ONECDEC_TOOLS = os.environ.get('ONECDEC_TOOLS', os.path.join(BESIDE, '1c-tools', 'onecdec'))
EXE = os.environ.get('IBCMD_EXE', os.path.join(
    REPO, 'target', 'release', 'ibcmd-rs.exe' if os.name == 'nt' else 'ibcmd-rs'))
WORK = os.environ.get('IBCMD_WORK', os.path.join(BESIDE, 'ibcmd-rs-work'))
CORPUS = os.environ.get('IBCMD_CORPUS', os.path.join(BESIDE, 'ibcmd-rs-corpus'))
