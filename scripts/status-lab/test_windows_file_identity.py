"""Native metadata ABI models only; these do not execute Windows APIs."""
import ctypes as c
import unittest
from unittest.mock import Mock, patch
import windows_file_identity as native


class FakeKernel:
    def __init__(self):
        self.fields = {"volume": 11, "id": (1 << 100) + 19, "size": 23, "written": 29,
                       "changed": 31, "created": 37, "links": 1, "attributes": 0,
                       "directory": 0, "delete_pending": 0}
        self.fail = None
        self.closed = []
        self.open_args = None
    def CreateFileW(self, *args):
        self.open_args = args
        return 17
    def CloseHandle(self, handle):
        self.closed.append(handle)
        return 1
    def GetFileInformationByHandleEx(self, handle, kind, pointer, size):
        if self.fail == kind: return 0
        cls = {0: native.Basic, 1: native.Standard, 18: native.FileId}[kind]
        assert size == c.sizeof(cls)
        value = c.cast(pointer, c.POINTER(cls)).contents
        if kind == 0:
            for name in ("written", "changed", "created", "attributes"): setattr(value, name, self.fields[name])
        elif kind == 1:
            for name in ("size", "links", "directory", "delete_pending"): setattr(value, name, self.fields[name])
        else:
            value.volume = self.fields["volume"]
            value.identifier[:] = self.fields["id"].to_bytes(16, "little")
        return 1


class NativeIdentityModel(unittest.TestCase):
    def test_legacy_diagnostic_returns_only_finite_field_names(self):
        from windows_rpc_gate import legacy_stat_mismatch_fields
        from types import SimpleNamespace
        named = SimpleNamespace(st_dev="private-shaped-marker", st_ctime_ns=11)
        opened = SimpleNamespace(st_dev="other-shaped-marker", st_ctime_ns=17)
        self.assertEqual(legacy_stat_mismatch_fields(named, opened), ["st_dev", "st_ctime_ns"])
    def test_fixed_windows_structure_widths(self):
        self.assertEqual([c.sizeof(t) for t in (native.Basic,native.Standard,native.FileId)], [40,24,24])
    def test_path_and_handle_share_full_metadata_witness(self):
        kernel = FakeKernel()
        expected = (11,(1 << 100)+19,23,29,31,37,1)
        with patch("windows_file_identity._kernel", return_value=kernel):
            self.assertEqual(native.path_identity("public-owned-fake"), expected)
        self.assertEqual(native._witness(17,kernel), expected)
        self.assertEqual(kernel.closed,[17])
        self.assertEqual(kernel.open_args, ("public-owned-fake",0x80,7,None,3,0x00200000,None))
    def test_borrowed_descriptor_uses_native_handle_without_closing_it(self):
        kernel = FakeKernel()
        crt = Mock(); crt.get_osfhandle.return_value = 17
        with patch.dict("sys.modules", {"msvcrt": crt}), patch("windows_file_identity._kernel", return_value=kernel):
            self.assertEqual(native.fd_identity(23), native._witness(17,kernel))
        crt.get_osfhandle.assert_called_once_with(23)
        self.assertEqual(kernel.closed, [])
    def test_failed_path_open_never_queries_or_closes_an_invalid_handle(self):
        kernel = FakeKernel(); kernel.CreateFileW = Mock(return_value=c.c_void_p(-1).value)
        kernel.GetFileInformationByHandleEx = Mock()
        with patch("windows_file_identity._kernel", return_value=kernel), self.assertRaisesRegex(ValueError,"open-refused"):
            native.path_identity("public-owned-fake")
        kernel.GetFileInformationByHandleEx.assert_not_called()
        self.assertEqual(kernel.closed, [])
    def test_every_identity_mutation_changes_witness(self):
        kernel = FakeKernel(); original = native._witness(17,kernel)
        for name in ("volume","id","size","written","changed","created","links"):
            previous = kernel.fields[name];kernel.fields[name] += 1
            self.assertNotEqual(native._witness(17,kernel),original,name)
            kernel.fields[name] = previous
    def test_unknown_metadata_refuses_and_closes_owned_path_handle(self):
        for kind in (0,1,18):
            kernel=FakeKernel();kernel.fail=kind
            with patch("windows_file_identity._kernel",return_value=kernel),self.assertRaisesRegex(ValueError,"metadata-unavailable"):
                native.path_identity("public-owned-fake")
            self.assertEqual(kernel.closed,[17])
    def test_reparse_directory_deleted_and_invalid_metadata_refuse(self):
        for name,value in (("attributes",0x400),("attributes",0x10),("directory",1),("delete_pending",1),("size",-1),("links",0)):
            kernel=FakeKernel();kernel.fields[name]=value
            with self.assertRaisesRegex(ValueError,"regular-file-required"):
                native._witness(17,kernel)
    def test_close_failure_cannot_produce_path_identity(self):
        kernel=FakeKernel();kernel.CloseHandle=Mock(return_value=0)
        with patch("windows_file_identity._kernel",return_value=kernel),self.assertRaisesRegex(ValueError,"close-unconfirmed"):
            native.path_identity("public-owned-fake")


if __name__ == "__main__": unittest.main()
