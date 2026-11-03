"""Read-only Windows file witnesses from one native metadata API family.

Only the reviewed fake harness uses this helper. No stat/fstat field mixing,
file data writes, arbitrary process execution or unsupported-profile fallback.
"""
import ctypes as c
import os


class Basic(c.Structure):
    _fields_ = [(name, c.c_int64) for name in ("created", "accessed", "written", "changed")] + [("attributes", c.c_uint32)]


class Standard(c.Structure):
    _fields_ = [("allocation", c.c_int64), ("size", c.c_int64), ("links", c.c_uint32),
                ("delete_pending", c.c_ubyte), ("directory", c.c_ubyte)]


class FileId(c.Structure):
    _fields_ = [("volume", c.c_uint64), ("identifier", c.c_ubyte * 16)]


_KERNEL = None


def _kernel():
    global _KERNEL
    if os.name != "nt":
        raise ValueError("Windows file metadata required")
    if _KERNEL is None:
        k = c.WinDLL("kernel32", use_last_error=True)
        k.CreateFileW.argtypes = [c.c_wchar_p, c.c_uint32, c.c_uint32, c.c_void_p, c.c_uint32, c.c_uint32, c.c_void_p]
        k.CreateFileW.restype = c.c_void_p
        k.GetFileInformationByHandleEx.argtypes = [c.c_void_p, c.c_int, c.c_void_p, c.c_uint32]
        k.GetFileInformationByHandleEx.restype = c.c_int
        k.CloseHandle.argtypes = [c.c_void_p]
        k.CloseHandle.restype = c.c_int
        _KERNEL = k
    return _KERNEL


def _witness(handle, kernel):
    basic, standard, file_id = Basic(), Standard(), FileId()
    for kind, result in ((0, basic), (1, standard), (18, file_id)):
        if not kernel.GetFileInformationByHandleEx(handle, kind, c.byref(result), c.sizeof(result)):
            raise ValueError("native-file-metadata-unavailable")
    if basic.attributes & (0x10 | 0x400) or standard.directory or standard.delete_pending or standard.size < 0 or standard.links < 1:
        raise ValueError("native-regular-file-required")
    # Ignore access time, which ordinary reads can change. Preserve full 128-bit
    # ID/volume, bytes, data-write/change/creation times and actual link count.
    return (int(file_id.volume), int.from_bytes(bytes(file_id.identifier), "little"),
            int(standard.size), int(basic.written), int(basic.changed), int(basic.created), int(standard.links))


def path_identity(path):
    kernel = _kernel()
    # FILE_READ_ATTRIBUTES; share read/write/delete for the deliberate fixture
    # drift controls. OPEN_REPARSE_POINT refuses final link traversal below.
    handle = kernel.CreateFileW(str(path), 0x80, 7, None, 3, 0x00200000, None)
    if handle is None or handle == c.c_void_p(-1).value:
        raise ValueError("native-file-open-refused")
    try:
        return _witness(handle, kernel)
    finally:
        if not kernel.CloseHandle(handle):
            raise ValueError("native-file-close-unconfirmed")


def fd_identity(fd):
    import msvcrt
    # Borrow the live Python descriptor's handle; do not close that capability.
    return _witness(msvcrt.get_osfhandle(fd), _kernel())
