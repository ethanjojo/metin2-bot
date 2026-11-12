//! The staged elevated helper must never load code from a medium-user-writable
//! directory. Seal it at creation, including OWNER_RIGHTS on inherited children.
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Directory(Option<PathBuf>);
impl Directory {
    pub(super) fn path(&self) -> &Path {
        self.0.as_deref().expect("owned staging directory")
    }
    pub(super) fn keep(mut self) -> PathBuf {
        self.0.take().expect("owned staging directory")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Some(path) = self.0.take()
            && path.parent().and_then(|p| p.canonicalize().ok())
                == std::env::temp_dir().canonicalize().ok()
            && crate::file_tree::files(&path).is_ok()
        {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::c_void,
        mem::size_of,
        os::windows::ffi::OsStrExt,
        ptr::{addr_of_mut, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, LocalFree},
        Security::{Authorization::*, *},
        Storage::FileSystem::{CreateDirectoryW, READ_CONTROL},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    struct Allocation(*mut c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn token() -> Result<Token, String> {
        let mut handle = null_mut();
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE,
                &mut handle,
            )
        } == 0
        {
            return Err("The Windows account could not be verified.".into());
        }
        Ok(Token(handle))
    }
    fn sid_string(sid: PSID) -> Result<String, String> {
        let mut text = null_mut();
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
            return Err("The Windows account SID is invalid.".into());
        }
        let allocation = Allocation(text.cast());
        let mut length = 0;
        while unsafe { *text.add(length) } != 0 {
            length += 1;
            if length > 256 {
                return Err("Windows SID is too long.".into());
            }
        }
        let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| "Invalid Windows SID.")?;
        drop(allocation);
        Ok(value)
    }
    pub(super) fn user_sid() -> Result<String, String> {
        let token = token()?;
        let mut needed = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed);
        }
        if needed == 0 || needed > 65536 {
            return Err("The Windows account token is invalid.".into());
        }
        let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err("The Windows account token is unavailable.".into());
        }
        sid_string(unsafe { (*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid })
    }
    fn elevated() -> Result<bool, String> {
        let token = token()?;
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut needed = 0;
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut needed,
            )
        } == 0
        {
            return Err("Windows elevation state is unavailable.".into());
        }
        Ok(elevation.TokenIsElevated != 0)
    }
    fn sddl(user: &str) -> String {
        format!("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;RC;;;OW)(A;OICI;GRGX;;;{user})")
    }
    fn descriptor(value: &str) -> Result<Allocation, String> {
        let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let mut output = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                1,
                &mut output,
                null_mut(),
            )
        } == 0
        {
            return Err("Windows could not create the protected update permissions.".into());
        }
        Ok(Allocation(output))
    }
    pub(super) fn create(id: uuid::Uuid) -> Result<Directory, String> {
        if !elevated()? {
            return Err("Open the installed Companion with administrator permissions before updating. No installation was started.".into());
        }
        let user = user_sid()?;
        let descriptor = descriptor(&sddl(&user))?;
        let path = std::env::temp_dir().join(format!(
            "companion-update-session-{id}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        crate::file_tree::reject_linked_ancestors(
            path.parent().ok_or("Missing temporary directory.")?,
        )
        .map_err(|e| e.to_string())?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) } == 0 {
            return Err(format!(
                "The protected update directory could not be created: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Directory(Some(path)))
    }
    fn verify_descriptor(
        descriptor: PSECURITY_DESCRIPTOR,
        user: &str,
        root: bool,
        directory: bool,
    ) -> Result<(), String> {
        let mut owner = null_mut();
        let mut defaulted = 0;
        if unsafe { GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted) } == 0 {
            return Err("Update ownership could not be checked.".into());
        }
        let owner = sid_string(owner)?;
        if (root && owner != "S-1-5-32-544") || (!root && owner != "S-1-5-32-544" && owner != user)
        {
            return Err("Unexpected update staging owner.".into());
        }
        let mut control = 0u16;
        let mut revision = 0;
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
            || (root && control & SE_DACL_PROTECTED == 0)
        {
            return Err("The update directory is not protected from inherited permissions.".into());
        }
        let mut present = 0;
        let mut acl = null_mut();
        let mut defaulted = 0;
        if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) }
            == 0
            || present == 0
            || acl.is_null()
        {
            return Err("The update directory has no restrictive access list.".into());
        }
        let count = unsafe { (*acl).AceCount };
        if count != 4 {
            return Err("Unexpected access rules in update staging.".into());
        }
        let mut found = std::collections::BTreeSet::new();
        for index in 0..u32::from(count) {
            let mut raw = null_mut();
            if unsafe { GetAce(acl, index, &mut raw) } == 0 {
                return Err("An update access rule could not be read.".into());
            }
            let ace = raw.cast::<ACCESS_ALLOWED_ACE>();
            if unsafe { (*ace).Header.AceType } != 0 {
                return Err("Unexpected update access rule type.".into());
            }
            let flags = unsafe { (*ace).Header.AceFlags };
            if flags & 0x08 != 0 || (directory && flags & 0x03 != 0x03) {
                return Err(
                    "Update staging contains ineffective or noninheritable access rules.".into(),
                );
            }
            let mask = unsafe { (*ace).Mask };
            let sid = sid_string(unsafe { addr_of_mut!((*ace).SidStart).cast() })?;
            let correct = match sid.as_str() {
                "S-1-5-18" | "S-1-5-32-544" => mask == 0x001f01ff,
                "S-1-3-4" => mask == READ_CONTROL,
                value if value == user => mask == 0xa0000000 || mask == 0x001200a9,
                _ => false,
            };
            if !correct || !found.insert(sid) {
                return Err("Update staging grants unexpected access.".into());
            }
        }
        Ok(())
    }
    pub(super) fn verify(path: &Path, root: bool) -> Result<(), String> {
        let user = user_sid()?;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut security = null_mut();
        let error = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                &mut security,
            )
        };
        if error != 0 {
            return Err("The update staging permissions could not be checked.".into());
        }
        let allocation = Allocation(security);
        let result = verify_descriptor(allocation.0, &user, root, path.is_dir());
        drop(allocation);
        result
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::{
            Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE},
            Storage::FileSystem::{CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL},
        };

        fn user_owned_fixture(path: &Path, user: &str) {
            // Set ownership at creation. A later OWNER_SECURITY_INFORMATION
            // update deliberately deactivates OWNER_RIGHTS ACEs on Windows.
            let owner = descriptor(&format!("O:{user}")).unwrap();
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: owner.0,
                bInheritHandle: 0,
            };
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_WRITE,
                    0,
                    &attributes,
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL,
                    null_mut(),
                )
            };
            assert_ne!(
                handle,
                INVALID_HANDLE_VALUE,
                "{}",
                std::io::Error::last_os_error()
            );
            let mut file = unsafe { fs::File::from_raw_handle(handle) };
            std::io::Write::write_all(&mut file, b"original child data").unwrap();
        }

        fn fixture_security(path: &Path) -> Allocation {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut security = null_mut();
            assert_eq!(
                unsafe {
                    GetNamedSecurityInfoW(
                        wide.as_ptr(),
                        SE_FILE_OBJECT,
                        OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                        null_mut(),
                        &mut security,
                    )
                },
                0
            );
            Allocation(security)
        }

        fn owner_rights_flags(security: &Allocation, user: &str) -> u8 {
            let mut owner = null_mut();
            let mut defaulted = 0;
            assert_ne!(
                unsafe { GetSecurityDescriptorOwner(security.0, &mut owner, &mut defaulted) },
                0
            );
            assert_eq!(sid_string(owner).unwrap(), user);
            let mut present = 0;
            let mut acl = null_mut();
            assert_ne!(
                unsafe {
                    GetSecurityDescriptorDacl(security.0, &mut present, &mut acl, &mut defaulted)
                },
                0
            );
            assert_ne!(present, 0);
            for index in 0..u32::from(unsafe { (*acl).AceCount }) {
                let mut raw = null_mut();
                assert_ne!(unsafe { GetAce(acl, index, &mut raw) }, 0);
                let ace = raw.cast::<ACCESS_ALLOWED_ACE>();
                if sid_string(unsafe { addr_of_mut!((*ace).SidStart).cast() }).unwrap() == "S-1-3-4"
                {
                    return unsafe { (*ace).Header.AceFlags };
                }
            }
            panic!("The fixture must inherit OWNER_RIGHTS");
        }

        #[test]
        fn initial_owner_preserves_owner_rights_but_later_assignment_deactivates_it() {
            let directory = tempfile::tempdir().unwrap();
            let user = user_sid().unwrap();
            // Only this disposable fixture grants its user full access, so the
            // owner-only mutation can also be reproduced without elevation.
            let permissions =
                descriptor(&format!("D:P(A;OICI;FA;;;{user})(A;OICI;RC;;;OW)")).unwrap();
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = null_mut();
            assert_ne!(
                unsafe {
                    GetSecurityDescriptorDacl(permissions.0, &mut present, &mut acl, &mut defaulted)
                },
                0
            );
            let root: Vec<u16> = directory
                .path()
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            assert_eq!(
                unsafe {
                    SetNamedSecurityInfoW(
                        root.as_ptr(),
                        SE_FILE_OBJECT,
                        DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                        null_mut(),
                        null_mut(),
                        acl,
                        null_mut(),
                    )
                },
                0
            );
            let child = directory.path().join("owner-child.dll");
            user_owned_fixture(&child, &user);
            assert_eq!(
                owner_rights_flags(&fixture_security(&child), &user) & 0x08,
                0
            );
            let wide: Vec<u16> = child.as_os_str().encode_wide().chain(Some(0)).collect();
            let user_text: Vec<u16> = user.encode_utf16().chain(Some(0)).collect();
            let mut sid = null_mut();
            assert_ne!(
                unsafe { ConvertStringSidToSidW(user_text.as_ptr(), &mut sid) },
                0
            );
            let _sid = Allocation(sid);
            assert_eq!(
                unsafe {
                    SetNamedSecurityInfoW(
                        wide.as_ptr(),
                        SE_FILE_OBJECT,
                        OWNER_SECURITY_INFORMATION,
                        sid,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                    )
                },
                0
            );
            assert_ne!(
                owner_rights_flags(&fixture_security(&child), &user) & 0x08,
                0
            );
        }

        fn restricted_primary() -> Token {
            let original = token().unwrap();
            let mut admin_sid = null_mut();
            let sid_text: Vec<u16> = "S-1-5-32-544\0".encode_utf16().collect();
            assert_ne!(
                unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut admin_sid) },
                0
            );
            let _sid_memory = Allocation(admin_sid);
            let disabled = SID_AND_ATTRIBUTES {
                Sid: admin_sid,
                Attributes: 0,
            };
            let mut restricted = null_mut();
            assert_ne!(
                unsafe {
                    CreateRestrictedToken(
                        original.0,
                        0,
                        1,
                        &disabled,
                        0,
                        std::ptr::null(),
                        0,
                        std::ptr::null(),
                        &mut restricted,
                    )
                },
                0
            );
            Token(restricted)
        }
        fn impersonation(primary: &Token) -> Token {
            let mut handle = null_mut();
            assert_ne!(
                unsafe { DuplicateToken(primary.0, SecurityImpersonation, &mut handle) },
                0
            );
            Token(handle)
        }
        fn medium_integrity(primary: &Token) {
            let text: Vec<u16> = "S-1-16-8192\0".encode_utf16().collect();
            let mut sid = null_mut();
            assert_ne!(
                unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut sid) },
                0
            );
            let _allocation = Allocation(sid);
            let sid_length = unsafe { GetLengthSid(sid) } as usize;
            let size = size_of::<TOKEN_MANDATORY_LABEL>() + sid_length;
            let mut buffer = vec![0usize; size.div_ceil(size_of::<usize>())];
            let label = buffer.as_mut_ptr().cast::<TOKEN_MANDATORY_LABEL>();
            let target = unsafe {
                buffer
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(size_of::<TOKEN_MANDATORY_LABEL>())
            }
            .cast();
            assert_ne!(unsafe { CopySid(sid_length as u32, target, sid) }, 0);
            unsafe {
                std::ptr::write(
                    label,
                    TOKEN_MANDATORY_LABEL {
                        Label: SID_AND_ATTRIBUTES {
                            Sid: target,
                            Attributes: 0x20, // SE_GROUP_INTEGRITY
                        },
                    },
                );
            }
            assert_ne!(
                unsafe {
                    SetTokenInformation(primary.0, TokenIntegrityLevel, label.cast(), size as u32)
                },
                0,
                "could not lower the restricted token to medium integrity"
            );
        }
        struct Impersonation;
        impl Impersonation {
            fn enter(token: &Token) -> Self {
                assert_ne!(unsafe { ImpersonateLoggedOnUser(token.0) }, 0);
                Self
            }
        }
        impl Drop for Impersonation {
            fn drop(&mut self) {
                if unsafe { RevertToSelf() } == 0 {
                    std::process::abort();
                }
            }
        }

        #[test]
        fn restricted_user_cannot_write_add_delete_or_change_permissions_even_as_child_owner() {
            let user = user_sid().unwrap();
            let restricted = restricted_primary();
            let impersonation = impersonation(&restricted);
            let sealed = descriptor(&sddl(&user).replace("O:BA", &format!("O:{user}"))).unwrap();
            let unsafe_owner = descriptor(
                &sddl(&user)
                    .replace("O:BA", &format!("O:{user}"))
                    .replace("(A;OICI;RC;;;OW)", ""),
            )
            .unwrap();
            let generic = GENERIC_MAPPING {
                GenericRead: 0x00120089,
                GenericWrite: 0x00120116,
                GenericExecute: 0x001200a0,
                GenericAll: 0x001f01ff,
            };
            let mut baseline_privileges = [0usize; 512];
            let mut baseline_size = std::mem::size_of_val(&baseline_privileges) as u32;
            let mut baseline_granted = 0;
            let mut baseline_allowed = 0;
            assert_ne!(
                unsafe {
                    AccessCheck(
                        unsafe_owner.0,
                        impersonation.0,
                        0x00040000,
                        &generic,
                        baseline_privileges.as_mut_ptr().cast(),
                        &mut baseline_size,
                        &mut baseline_granted,
                        &mut baseline_allowed,
                    )
                },
                0
            );
            assert_eq!(
                baseline_allowed, 1,
                "control descriptor must expose implicit owner WRITE_DAC without OWNER_RIGHTS"
            );
            for right in [0x00000002, 0x00000004, 0x00010000, 0x00040000, 0x00080000] {
                let mut privileges = [0usize; 512];
                let mut size = std::mem::size_of_val(&privileges) as u32;
                let mut granted = 0;
                let mut allowed = 0;
                assert_ne!(
                    unsafe {
                        AccessCheck(
                            sealed.0,
                            impersonation.0,
                            right,
                            &generic,
                            privileges.as_mut_ptr().cast(),
                            &mut size,
                            &mut granted,
                            &mut allowed,
                        )
                    },
                    0
                );
                assert_eq!(
                    allowed, 0,
                    "restricted child owner unexpectedly granted access {right:x}"
                );
            }
        }

        /// Run only on an already-elevated disposable Windows runner. This
        /// exercises TEMP ACL inheritance, never Setup, drivers or game inputs.
        #[test]
        #[ignore = "Requires already-elevated isolated Windows acceptance"]
        fn sealed_staging_actual_acl() {
            assert!(
                elevated().unwrap(),
                "Run this acceptance test from an already-elevated isolated Windows runner; it never requests UAC."
            );
            let directory = create(uuid::Uuid::new_v4()).unwrap();
            let root = directory.path().to_path_buf();
            let root_lock = crate::update_helper::session::locked_directory(&root).unwrap();
            let runtime = root.join("runtime");
            let nested = runtime.join("nested");
            fs::create_dir(&runtime).unwrap();
            fs::create_dir(&nested).unwrap();
            let executable = runtime.join("Companion.exe");
            let child = nested.join("owner-child.dll");
            fs::write(&executable, b"nonexecutable fixture").unwrap();
            let user = user_sid().unwrap();
            user_owned_fixture(&child, &user);
            verify(&root, true).unwrap();
            for path in [&runtime, &nested, &executable, &child] {
                verify(path, false).unwrap();
            }
            // The owner was explicitly assigned at creation, independently of
            // the elevated token's default owner, retaining effective protection.
            assert_eq!(
                owner_rights_flags(&fixture_security(&child), &user) & 0x08,
                0
            );
            let user_text: Vec<u16> = user.encode_utf16().chain(Some(0)).collect();
            let mut user_sid = null_mut();
            assert_ne!(
                unsafe { ConvertStringSidToSidW(user_text.as_ptr(), &mut user_sid) },
                0
            );
            let _user = Allocation(user_sid);
            let child_wide: Vec<u16> = child.as_os_str().encode_wide().chain(Some(0)).collect();
            verify(&child, false).unwrap();
            let restricted = restricted_primary();
            medium_integrity(&restricted);
            let medium = impersonation(&restricted);
            let injection = runtime.join("vulkan-1.dll");
            {
                let _identity = Impersonation::enter(&medium);
                let denied = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&injection)
                    .expect_err("A medium token must not add a new loader DLL");
                assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
                let denied = fs::OpenOptions::new()
                    .write(true)
                    .open(&child)
                    .expect_err("A medium child owner must not overwrite a staged file");
                assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(
                    fs::remove_file(&child).unwrap_err().kind(),
                    std::io::ErrorKind::PermissionDenied
                );
                // Removing the DACL would grant everyone access if WRITE_DAC
                // were accidentally inherited through ownership.
                let denied = unsafe {
                    SetNamedSecurityInfoW(
                        child_wide.as_ptr(),
                        SE_FILE_OBJECT,
                        DACL_SECURITY_INFORMATION,
                        null_mut(),
                        null_mut(),
                        std::ptr::null(),
                        std::ptr::null(),
                    )
                };
                assert_eq!(denied, 5, "medium owner unexpectedly changed the ACL");
                assert_eq!(
                    fs::read(&child).unwrap(),
                    b"original child data",
                    "read-only diagnostics access should remain available"
                );
            }
            assert!(!injection.exists());
            assert_eq!(fs::read(&child).unwrap(), b"original child data");
            verify(&child, false).unwrap();
            // A later owner assignment disables OWNER_RIGHTS. The production
            // verifier must continue rejecting that altered security descriptor.
            assert_eq!(
                unsafe {
                    SetNamedSecurityInfoW(
                        child_wide.as_ptr(),
                        SE_FILE_OBJECT,
                        OWNER_SECURITY_INFORMATION,
                        user_sid,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                    )
                },
                0
            );
            assert!(verify(&child, false).unwrap_err().contains("ineffective"));
            drop(root_lock);
            drop(directory);
            assert!(
                !root.exists(),
                "the protected TEMP fixture must clean up after reverting impersonation"
            );
        }

        #[test]
        fn descriptor_allows_only_administrators_system_and_read_only_owner_user() {
            let user = user_sid().unwrap();
            let sealed = descriptor(&sddl(&user)).unwrap();
            assert!(verify_descriptor(sealed.0, &user, true, true).is_ok());
            let unsafe_acl = descriptor(&format!(
                "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;RC;;;OW)(A;OICI;FA;;;{user})"
            ))
            .unwrap();
            assert!(verify_descriptor(unsafe_acl.0, &user, true, true).is_err());
            let child = descriptor(&sddl(&user).replace("O:BA", &format!("O:{user}"))).unwrap();
            assert!(verify_descriptor(child.0, &user, false, true).is_ok());
            assert!(verify_descriptor(child.0, &user, true, true).is_err());
            let ineffective =
                descriptor(&sddl(&user).replace("OICI;RC;;;OW", "OICIIO;RC;;;OW")).unwrap();
            assert!(verify_descriptor(ineffective.0, &user, true, true).is_err());
            let uninherited = descriptor(&sddl(&user).replace("OICI;RC;;;OW", ";RC;;;OW")).unwrap();
            assert!(verify_descriptor(uninherited.0, &user, true, true).is_err());
        }
    }
}

pub(super) fn create(id: uuid::Uuid) -> Result<Directory, String> {
    #[cfg(windows)]
    {
        windows::create(id)
    }
    #[cfg(not(windows))]
    {
        let _ = id;
        Err("Updates require Windows.".into())
    }
}
pub(super) fn verify(path: &Path, root: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::verify(path, root)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, root);
        Err("Updates require Windows.".into())
    }
}

/// wgpu may load Vulkan by bare filename. Exclude inherited CWD/PATH from that
/// search; only the sealed executable directory and Windows System32 are used.
pub(super) fn restrict_dll_search() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{
            LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
            SetDefaultDllDirectories,
        };
        if unsafe {
            SetDefaultDllDirectories(
                LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        } == 0
        {
            return Err("The update helper could not secure its DLL search path.".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Updates require Windows.".into())
    }
}
