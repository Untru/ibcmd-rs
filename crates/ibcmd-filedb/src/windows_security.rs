//! Handle-only owner/group/DACL equality, not SACL/audit/mandatory-label preservation.
//! Unknown or different security refuses before database bytes reach a new file.
#![allow(unsafe_code)]

use crate::{Error, Result};
use std::{ffi::c_void, fs::File, os::windows::io::AsRawHandle, ptr::NonNull};

const SE_FILE_OBJECT: i32 = 1;
const OWNER_SECURITY_INFORMATION: u32 = 0x1;
const GROUP_SECURITY_INFORMATION: u32 = 0x2;
const DACL_SECURITY_INFORMATION: u32 = 0x4;
const SE_OWNER_DEFAULTED: u16 = 0x1;
const SE_GROUP_DEFAULTED: u16 = 0x2;
const SE_DACL_PRESENT: u16 = 0x4;
const SE_DACL_DEFAULTED: u16 = 0x8;
const SE_DACL_AUTO_INHERIT_REQ: u16 = 0x100;
const SE_DACL_AUTO_INHERITED: u16 = 0x400;
const SE_DACL_PROTECTED: u16 = 0x1000;
const DACL_CONTROL: u16 = SE_OWNER_DEFAULTED
    | SE_GROUP_DEFAULTED
    | SE_DACL_PRESENT
    | SE_DACL_DEFAULTED
    | SE_DACL_AUTO_INHERIT_REQ
    | SE_DACL_AUTO_INHERITED
    | SE_DACL_PROTECTED;
const ACL_SIZE_INFORMATION_CLASS: i32 = 2;

// SDK ACL_SIZE_INFORMATION: three DWORDs, not a SECURITY_DESCRIPTOR layout.
#[repr(C)]
#[derive(Default)]
struct AclSizeInformation {
    ace_count: u32,
    bytes_in_use: u32,
    bytes_free: u32,
}
const _: [(); 12] = [(); std::mem::size_of::<AclSizeInformation>()];
const _: [(); 4] = [(); std::mem::align_of::<AclSizeInformation>()];

#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        handle: *mut c_void,
        object_type: i32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn IsValidSecurityDescriptor(descriptor: *mut c_void) -> i32;
    fn EqualSid(first: *mut c_void, second: *mut c_void) -> i32;
    fn IsValidSid(sid: *mut c_void) -> i32;
    fn GetSecurityDescriptorControl(
        descriptor: *mut c_void,
        control: *mut u16,
        revision: *mut u32,
    ) -> i32;
    fn GetSecurityDescriptorDacl(
        descriptor: *mut c_void,
        present: *mut i32,
        dacl: *mut *mut c_void,
        defaulted: *mut i32,
    ) -> i32;
    fn IsValidAcl(acl: *mut c_void) -> i32;
    fn GetAclInformation(
        acl: *mut c_void,
        information: *mut c_void,
        length: u32,
        class: i32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

#[derive(Debug, PartialEq, Eq)]
enum Dacl {
    Absent,
    Null,
    Present(Vec<u8>),
}
#[derive(Debug, PartialEq, Eq)]
struct AccessControl {
    flags: u16,
    dacl: Dacl,
}

struct Descriptor {
    allocation: NonNull<c_void>,
    owner: NonNull<c_void>,
    group: NonNull<c_void>,
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: GetSecurityInfo owns this LocalAlloc allocation; no field outlives it.
        unsafe {
            LocalFree(self.allocation.as_ptr());
        }
    }
}
impl Descriptor {
    fn read(file: &File) -> Result<Self> {
        let (mut owner, mut group, mut descriptor) = (
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        // SAFETY: live owned file handle and initialized, correctly typed output pointers.
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                &mut group,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(Error::Io(std::io::Error::from_raw_os_error(status as i32)));
        }
        let allocation = NonNull::new(descriptor)
            .ok_or_else(|| Error::Refused("missing file security descriptor".into()))?;
        // Keep allocation RAII-owned even if a requested SID is missing/invalid.
        struct Allocation(NonNull<c_void>);
        impl Drop for Allocation {
            fn drop(&mut self) {
                unsafe {
                    LocalFree(self.0.as_ptr());
                }
            }
        }
        let allocation = Allocation(allocation);
        let owner =
            NonNull::new(owner).ok_or_else(|| Error::Refused("missing file owner SID".into()))?;
        let group =
            NonNull::new(group).ok_or_else(|| Error::Refused("missing file group SID".into()))?;
        // SAFETY: descriptor and requested SIDs belong to the live OS allocation.
        if unsafe {
            IsValidSecurityDescriptor(allocation.0.as_ptr()) == 0
                || IsValidSid(owner.as_ptr()) == 0
                || IsValidSid(group.as_ptr()) == 0
        } {
            return Err(Error::Refused(
                "invalid file owner/group/DACL descriptor".into(),
            ));
        }
        let result = Self {
            allocation: allocation.0,
            owner,
            group,
        };
        std::mem::forget(allocation);
        Ok(result)
    }
    fn dacl(&self) -> Result<AccessControl> {
        let (mut control, mut revision, mut present, mut defaulted) = (0u16, 0u32, 0i32, 0i32);
        let mut acl = std::ptr::null_mut();
        // SAFETY: self keeps the validated descriptor alive; all outputs have SDK widths.
        if unsafe {
            GetSecurityDescriptorControl(self.allocation.as_ptr(), &mut control, &mut revision)
        } == 0
            || unsafe {
                GetSecurityDescriptorDacl(
                    self.allocation.as_ptr(),
                    &mut present,
                    &mut acl,
                    &mut defaulted,
                )
            } == 0
        {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        let flags = control & DACL_CONTROL;
        if present == 0 {
            return Ok(AccessControl {
                flags,
                dacl: Dacl::Absent,
            });
        }
        if acl.is_null() {
            return Ok(AccessControl {
                flags,
                dacl: Dacl::Null,
            });
        }
        let mut size = AclSizeInformation::default();
        // SAFETY: present non-NULL ACL belongs to self; SDK size query bounds its used bytes.
        if unsafe { IsValidAcl(acl) } == 0 {
            return Err(Error::Refused("invalid file DACL".into()));
        }
        if unsafe {
            GetAclInformation(
                acl,
                (&mut size as *mut AclSizeInformation).cast(),
                std::mem::size_of::<AclSizeInformation>() as u32,
                ACL_SIZE_INFORMATION_CLASS,
            )
        } == 0
        {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        // SAFETY: successful SDK query reports the bytes in use in this valid, live ACL.
        let bytes =
            unsafe { std::slice::from_raw_parts(acl.cast::<u8>(), size.bytes_in_use as usize) }
                .to_vec();
        Ok(AccessControl {
            flags,
            dacl: Dacl::Present(bytes),
        })
    }
}

pub(crate) fn require_same(original: &File, copy: &File) -> Result<()> {
    let original = Descriptor::read(original)?;
    let copy = Descriptor::read(copy)?;
    // SAFETY: both descriptors own validated SIDs throughout comparison.
    let equal = unsafe {
        EqualSid(original.owner.as_ptr(), copy.owner.as_ptr()) != 0
            && EqualSid(original.group.as_ptr(), copy.group.as_ptr()) != 0
    };
    if !equal || original.dacl()? != copy.dacl()? {
        return Err(Error::Refused(
            "file owner/group/DACL differs from original; refusing content copy".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        fs,
        io::{Read, Write},
        os::windows::fs::OpenOptionsExt,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertSidToStringSidW(sid: *mut c_void, text: *mut *mut u16) -> i32;
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            length: *mut u32,
        ) -> i32;
        fn SetSecurityInfo(
            handle: *mut c_void,
            object_type: i32,
            information: u32,
            owner: *mut c_void,
            group: *mut c_void,
            dacl: *mut c_void,
            sacl: *mut c_void,
        ) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn lstrlenW(text: *const u16) -> i32;
    }
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Scratch {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "filedb-acl-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/file-infobase/tiny.1CD.gz");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(File::open(source).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        fs::write(path.join("1Cv8.1CD"), bytes).unwrap();
        Scratch(path)
    }
    fn restrict_to_owner(path: &Path) {
        const READ_CONTROL: u32 = 0x20000;
        const WRITE_DAC: u32 = 0x40000;
        const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x80000000;
        let file = fs::OpenOptions::new()
            .access_mode(READ_CONTROL | WRITE_DAC)
            .open(path)
            .unwrap();
        let descriptor = Descriptor::read(&file).unwrap();
        let mut text = std::ptr::null_mut();
        // SAFETY: live validated owner SID; conversion allocates a NUL-terminated string.
        assert_ne!(
            unsafe { ConvertSidToStringSidW(descriptor.owner.as_ptr(), &mut text) },
            0
        );
        let owner = unsafe {
            String::from_utf16(std::slice::from_raw_parts(text, lstrlenW(text) as usize)).unwrap()
        };
        unsafe {
            LocalFree(text.cast());
        }
        let sddl = format!("D:P(A;;FA;;;{owner})")
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut protected = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut protected,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let (mut present, mut defaulted, mut acl) = (0, 0, std::ptr::null_mut());
        assert_ne!(
            unsafe { GetSecurityDescriptorDacl(protected, &mut present, &mut acl, &mut defaulted) },
            0
        );
        let status = unsafe {
            SetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            LocalFree(protected);
        }
        assert_eq!(status, 0);
        assert_ne!(
            Descriptor::read(&file).unwrap().dacl().unwrap().flags & SE_DACL_PROTECTED,
            0
        );
    }
    #[test]
    fn security_query_failure_is_not_treated_as_matching_defaults() {
        let scratch = fixture();
        let source = scratch.0.join("1Cv8.1CD");
        let metadata_only = fs::OpenOptions::new().access_mode(0).open(source).unwrap();
        let empty = File::create(scratch.0.join("empty")).unwrap();
        let error = require_same(&metadata_only, &empty).unwrap_err();
        assert!(
            matches!(error, Error::Io(ref error) if error.raw_os_error() == Some(5)),
            "{error}"
        );
        assert_eq!(empty.metadata().unwrap().len(), 0);
    }
    #[test]
    fn windows_restrictive_dacl_refuses_writer_before_database_bytes() {
        let scratch = fixture();
        let path = scratch.0.join("1Cv8.1CD");
        restrict_to_owner(&path);
        let before = fs::read(&path).unwrap();
        let before_dacl = Descriptor::read(&File::open(&path).unwrap())
            .unwrap()
            .dacl()
            .unwrap();
        let observed_bytes = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observer = observed_bytes.clone();
        let work = crate::sibling(&path, crate::WORK_SUFFIX);
        let session = crate::open_for_write_with_limits(
            &path,
            crate::ReadLimits {
                max_records: 1_000_000,
                max_binary_bytes: 1 << 30,
                max_object_bytes: 1 << 30,
            },
            move || {
                if fs::metadata(&work).is_ok_and(|metadata| metadata.len() != 0) {
                    observer.store(true, Ordering::SeqCst);
                }
                Ok(())
            },
            &mut |_| Ok(()),
        )
        .unwrap();
        let result = session.replace("CONFIGSAVE", &[]);
        if result.is_ok() {
            let after_dacl = Descriptor::read(&File::open(&path).unwrap())
                .unwrap()
                .dacl()
                .unwrap();
            assert_ne!(
                before_dacl, after_dacl,
                "failing-first probe must measure actual DACL loss"
            );
            eprintln!("observed actual protected source DACL replaced by inherited work DACL");
        }
        assert!(
            !observed_bytes.load(Ordering::SeqCst),
            "database bytes reached the less-restricted work file"
        );
        let error = result.unwrap_err();
        assert!(
            matches!(error, Error::Refused(_))
                && error.to_string().contains("owner/group/DACL differs"),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!crate::backup_file(&path).exists());
        assert!(!crate::sibling(&path, crate::WORK_SUFFIX).exists());
    }
    #[test]
    fn windows_changed_work_dacl_refuses_publication() {
        let scratch = fixture();
        let path = scratch.0.join("1Cv8.1CD");
        let before = fs::read(&path).unwrap();
        let work = crate::sibling(&path, crate::WORK_SUFFIX);
        let mut gates = 0;
        let result =
            crate::open_for_write(&path)
                .unwrap()
                .replace_guarded("CONFIGSAVE", &[], |_| {
                    gates += 1;
                    if gates == 2 {
                        restrict_to_owner(&work);
                    }
                    Ok(())
                });
        assert_eq!(gates, 2);
        let error = result.unwrap_err();
        assert!(
            matches!(error, Error::Refused(_))
                && error.to_string().contains("owner/group/DACL differs"),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!work.exists());
        assert!(!crate::backup_file(&path).exists());
        assert!(!crate::sibling(&path, ".ibcmd-rs.bak.new").exists());
    }
    #[test]
    fn windows_restrictive_dacl_refuses_backup_before_copy_callback() {
        let scratch = fixture();
        let path = scratch.0.join("1Cv8.1CD");
        restrict_to_owner(&path);
        let original = File::open(&path).unwrap();
        let pending = scratch.0.join("pending");
        let paths = crate::write_paths::WritePaths::open(&path).unwrap();
        let copied = Cell::new(false);
        let result = crate::prepare_backup_file(
            &paths,
            &original,
            &pending,
            || Err(std::io::Error::other("force fallback")),
            |out| {
                copied.set(true);
                assert!(
                    require_same(&original, out).is_err(),
                    "failing-first fallback must have different security"
                );
                eprintln!("observed fallback callback copying under a different inherited DACL");
                out.write_all(b"sensitive fixture bytes")?;
                Ok(())
            },
        );
        let error = result.unwrap_err();
        assert!(
            matches!(error, Error::Refused(_))
                && error.to_string().contains("owner/group/DACL differs"),
            "{error}"
        );
        assert!(
            !copied.get(),
            "security must be checked before copying any byte"
        );
        assert!(!pending.exists());
    }
}
