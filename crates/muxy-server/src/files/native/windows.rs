//! Each name opens relative to its parent's handle with `NtCreateFile`, never
//! following a symbolic link, junction or mount point. Renames and deletes
//! act on the opened handle.

use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io;
use std::mem;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_DISPOSITION_DELETE, FILE_DISPOSITION_INFORMATION,
    FILE_DISPOSITION_INFORMATION_EX, FILE_DISPOSITION_POSIX_SEMANTICS, FILE_NON_DIRECTORY_FILE,
    FILE_OPEN, FILE_OPEN_FOR_BACKUP_INTENT, FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION,
    FILE_RENAME_POSIX_SEMANTICS, FILE_RENAME_REPLACE_IF_EXISTS, FILE_SYNCHRONOUS_IO_NONALERT,
    FileDispositionInformation, FileDispositionInformationEx, FileRenameInformation,
    FileRenameInformationEx, NtCreateFile, NtSetInformationFile,
};
use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_FILES, GENERIC_READ, GENERIC_WRITE, HANDLE, OBJ_CASE_INSENSITIVE, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_FULL_DIR_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileAttributeTagInfo,
    FileFullDirectoryInfo, FileFullDirectoryRestartInfo, GetFileInformationByHandle,
    GetFileInformationByHandleEx, SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use super::{Entry, Kind};
use crate::platform::windows::status_error;

const DIRECTORY_ACCESS: u32 =
    FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

/// Reparse tags of this kind name another file: symbolic links, junctions
/// and mount points. Others, such as `OneDrive` placeholders, are the file.
const NAME_SURROGATE: u32 = 0x2000_0000;

pub(in crate::files) fn open_root(path: &Path) -> io::Result<File> {
    let directory = OpenOptions::new()
        .access_mode(DIRECTORY_ACCESS)
        .share_mode(SHARE_ALL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let (attributes, tag) = attribute_tag(&directory)?;
    if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::new(io::ErrorKind::NotADirectory, "not a folder"));
    }
    refuse_links(attributes, tag)?;
    Ok(directory)
}

pub(in crate::files) fn open_directory(parent: &File, name: &OsStr) -> io::Result<File> {
    let directory = open_at(
        parent,
        name,
        DIRECTORY_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    let (attributes, tag) = attribute_tag(&directory)?;
    refuse_links(attributes, tag)?;
    Ok(directory)
}

pub(in crate::files) fn stat(parent: &File, name: &OsStr) -> io::Result<Entry> {
    let file = open_at(
        parent,
        name,
        FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        FILE_OPEN,
        0,
    )?;
    let (attributes, tag) = attribute_tag(&file)?;
    let information = information(&file)?;
    Ok(Entry {
        kind: if is_link(attributes, tag) {
            Kind::Link
        } else if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            Kind::Directory
        } else {
            Kind::File
        },
        size: (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow),
        device: u64::from(information.dwVolumeSerialNumber),
        index: (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    })
}

/// Windows never follows links inside a project, so there is no target.
pub(in crate::files) fn read_link(_parent: &File, _name: &OsStr) -> io::Result<PathBuf> {
    Err(link_refused())
}

/// Opens a file for reading. A placeholder that a sync tool such as
/// `OneDrive` keeps is opened again normally, so its content downloads.
pub(in crate::files) fn open_file(parent: &File, name: &OsStr) -> io::Result<File> {
    let access = GENERIC_READ | SYNCHRONIZE;
    let file = open_at(parent, name, access, FILE_OPEN, FILE_NON_DIRECTORY_FILE)?;
    let (attributes, tag) = attribute_tag(&file)?;
    refuse_links(attributes, tag)?;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return Ok(file);
    }
    let content = open_following(parent, name, access)?;
    if identity(&content)? != identity(&file)? {
        return Err(io::Error::other("File changed while opening it; retry"));
    }
    Ok(content)
}

pub(in crate::files) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    open_at(
        parent,
        name,
        GENERIC_WRITE | DELETE | SYNCHRONIZE,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE,
    )
}

/// Windows has no permission bits to carry over.
#[allow(clippy::unnecessary_wraps, reason = "Matches the Unix signature")]
pub(in crate::files) fn copy_mode(_file: &File, _replaced: &Entry) -> io::Result<()> {
    Ok(())
}

/// Replaces `to` with `from`, both in `parent`, even while `to` is open.
pub(in crate::files) fn replace(parent: &File, from: &OsStr, to: &OsStr) -> io::Result<()> {
    let source = open_for_change(parent, from)?;
    rename_handle(
        &source,
        parent,
        to,
        FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS,
    )
}

/// Renames without replacing anything; a taken name is `AlreadyExists`.
pub(in crate::files) fn rename(
    parent: &File,
    from: &OsStr,
    destination: &File,
    to: &OsStr,
) -> io::Result<()> {
    let source = open_for_change(parent, from)?;
    rename_handle(&source, destination, to, 0)
}

pub(in crate::files) fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    let file = open_at(
        parent,
        name,
        DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    let (attributes, tag) = attribute_tag(&file)?;
    refuse_links(attributes, tag)?;
    let disposition = FILE_DISPOSITION_INFORMATION_EX {
        Flags: FILE_DISPOSITION_DELETE | FILE_DISPOSITION_POSIX_SEMANTICS,
    };
    let deleted = set_information(
        &file,
        ptr::from_ref(&disposition).cast(),
        size_u32::<FILE_DISPOSITION_INFORMATION_EX>(),
        FileDispositionInformationEx,
    );
    if !deleted.as_ref().is_err_and(unsupported) {
        return deleted;
    }
    let disposition = FILE_DISPOSITION_INFORMATION { DeleteFile: true };
    set_information(
        &file,
        ptr::from_ref(&disposition).cast(),
        size_u32::<FILE_DISPOSITION_INFORMATION>(),
        FileDispositionInformation,
    )
}

/// Whether a volume refused an `Ex` information class or its flags, as
/// FAT32, exFAT and some network shares do.
fn unsupported(error: &io::Error) -> bool {
    use windows_sys::Win32::Foundation::{
        ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED,
    };
    error.raw_os_error().is_some_and(|code| {
        [
            ERROR_INVALID_FUNCTION,
            ERROR_INVALID_PARAMETER,
            ERROR_NOT_SUPPORTED,
        ]
        .contains(&code.cast_unsigned())
    })
}

pub(in crate::files) fn create_directory(parent: &File, name: &OsStr) -> io::Result<()> {
    open_at(
        parent,
        name,
        DIRECTORY_ACCESS,
        FILE_CREATE,
        FILE_DIRECTORY_FILE,
    )
    .map(drop)
}

/// The names in `directory`, without `.` and `..`.
pub(in crate::files) fn names(directory: &File) -> io::Result<Vec<OsString>> {
    let mut buffer = vec![0_u64; 8 * 1024];
    let size = u32::try_from(buffer.len() * 8).map_err(io::Error::other)?;
    let mut names = Vec::new();
    let mut class = FileFullDirectoryRestartInfo;
    loop {
        // SAFETY: `buffer` is a live, 8-byte aligned buffer of `size` bytes.
        let read = unsafe {
            GetFileInformationByHandleEx(
                directory.as_raw_handle(),
                class,
                buffer.as_mut_ptr().cast(),
                size,
            )
        };
        if read == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES.cast_signed()) {
                return Ok(names);
            }
            return Err(error);
        }
        class = FileFullDirectoryInfo;
        let bytes = buffer.len() * 8;
        let mut offset = 0;
        loop {
            let header = mem::offset_of!(FILE_FULL_DIR_INFO, FileName);
            if offset + header > bytes {
                return Err(io::Error::other("malformed directory listing"));
            }
            // SAFETY: the system wrote an entry at `offset`, which starts
            // on an 8-byte boundary inside the buffer and holds its header.
            let entry = unsafe {
                ptr::read_unaligned(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_FULL_DIR_INFO>(),
                )
            };
            let length = entry.FileNameLength as usize;
            if offset + header + length > bytes {
                return Err(io::Error::other("malformed directory listing"));
            }
            // SAFETY: the name's bytes were checked to lie inside the buffer,
            // and start on a 2-byte boundary: entries are 8-byte aligned and
            // the name follows a header of 4-byte fields.
            #[allow(
                clippy::cast_ptr_alignment,
                reason = "Aligned as the SAFETY comment says"
            )]
            let name = unsafe {
                std::slice::from_raw_parts(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset + header)
                        .cast::<u16>(),
                    length / 2,
                )
            };
            let name = OsString::from_wide(name);
            if name != "." && name != ".." {
                names.push(name);
            }
            if entry.NextEntryOffset == 0 {
                break;
            }
            offset += entry.NextEntryOffset as usize;
        }
    }
}

/// Opens `name` in `parent` for a rename or delete, refusing links.
fn open_for_change(parent: &File, name: &OsStr) -> io::Result<File> {
    let file = open_at(
        parent,
        name,
        DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        FILE_OPEN,
        0,
    )?;
    let (attributes, tag) = attribute_tag(&file)?;
    refuse_links(attributes, tag)?;
    Ok(file)
}

#[allow(
    clippy::cast_ptr_alignment,
    reason = "The name follows 4-byte header fields in an 8-byte aligned buffer"
)]
fn rename_handle(file: &File, destination: &File, name: &OsStr, flags: u32) -> io::Result<()> {
    let wide: Vec<u16> = name.encode_wide().collect();
    let header = mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
    let bytes = size_of::<FILE_RENAME_INFORMATION>() + wide.len() * 2;
    let mut buffer = vec![0_u64; bytes.div_ceil(8)];
    let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: the buffer is 8-byte aligned and holds the header followed by
    // the name, which starts on a 4-byte boundary.
    unsafe {
        (*information).Anonymous.Flags = flags;
        (*information).RootDirectory = destination.as_raw_handle();
        (*information).FileNameLength = u32::try_from(wide.len() * 2).map_err(io::Error::other)?;
        ptr::copy_nonoverlapping(
            wide.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(header).cast::<u16>(),
            wide.len(),
        );
    }
    let length = u32::try_from(bytes).map_err(io::Error::other)?;
    let renamed = set_information(
        file,
        buffer.as_ptr().cast(),
        length,
        FileRenameInformationEx,
    );
    if !renamed.as_ref().is_err_and(unsupported) {
        return renamed;
    }
    // The older class reads only whether to replace, from the same layout.
    // SAFETY: as above.
    unsafe {
        (*information).Anonymous.Flags = 0;
        (*information).Anonymous.ReplaceIfExists = flags & FILE_RENAME_REPLACE_IF_EXISTS != 0;
    }
    set_information(file, buffer.as_ptr().cast(), length, FileRenameInformation)
}

fn set_information(
    file: &File,
    information: *const std::ffi::c_void,
    length: u32,
    class: i32,
) -> io::Result<()> {
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: the caller passes a live structure of `length` bytes for `class`.
    let status = unsafe {
        NtSetInformationFile(
            file.as_raw_handle(),
            &raw mut status_block,
            information,
            length,
            class,
        )
    };
    if status < 0 {
        Err(status_error(status))
    } else {
        Ok(())
    }
}

/// `NtCreateFile` for `name` relative to `parent`, opening a reparse point
/// itself rather than what it points to.
fn open_at(
    parent: &File,
    name: &OsStr,
    access: u32,
    disposition: u32,
    options: u32,
) -> io::Result<File> {
    open_relative(
        parent,
        name,
        access,
        disposition,
        options | FILE_OPEN_REPARSE_POINT,
    )
}

/// As [`open_at`], but letting the file system follow the file's own
/// reparse point, as for a cloud placeholder.
fn open_following(parent: &File, name: &OsStr, access: u32) -> io::Result<File> {
    open_relative(parent, name, access, FILE_OPEN, FILE_NON_DIRECTORY_FILE)
}

fn open_relative(
    parent: &File,
    name: &OsStr,
    access: u32,
    disposition: u32,
    options: u32,
) -> io::Result<File> {
    // An empty name opens the root directory itself, as `.` does on Unix.
    let mut wide: Vec<u16> = if name == "." {
        Vec::new()
    } else {
        name.encode_wide().collect()
    };
    let length = u16::try_from(wide.len() * 2)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidFilename, "name is too long"))?;
    if name.is_empty() || wide.contains(&0) || wide.contains(&u16::from(b'\\')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidFilename,
            "invalid name",
        ));
    }
    let object_name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_u32::<OBJECT_ATTRIBUTES>(),
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &raw const object_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: ptr::null(),
        SecurityQualityOfService: ptr::null(),
    };
    let mut handle: HANDLE = ptr::null_mut();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: every pointer refers to a live value that outlives the call;
    // the name buffer is not written to.
    let status = unsafe {
        NtCreateFile(
            &raw mut handle,
            access,
            &raw const attributes,
            &raw mut status_block,
            ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            SHARE_ALL,
            disposition,
            options | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_FOR_BACKUP_INTENT,
            ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(status_error(status));
    }
    // SAFETY: NtCreateFile succeeded, so `handle` is a new handle owned here.
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn attribute_tag(file: &File) -> io::Result<(u32, u32)> {
    let mut information = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: `information` is a live structure of the size passed.
    check(unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            ptr::from_mut(&mut information).cast(),
            size_u32::<FILE_ATTRIBUTE_TAG_INFO>(),
        )
    })?;
    Ok((information.FileAttributes, information.ReparseTag))
}

fn information(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `information` is a live structure the call fills in.
    check(unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut information) })?;
    Ok(information)
}

fn identity(file: &File) -> io::Result<(u32, u32, u32)> {
    let information = information(file)?;
    Ok((
        information.dwVolumeSerialNumber,
        information.nFileIndexHigh,
        information.nFileIndexLow,
    ))
}

fn is_link(attributes: u32, tag: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && tag & NAME_SURROGATE != 0
}

fn refuse_links(attributes: u32, tag: u32) -> io::Result<()> {
    if is_link(attributes, tag) {
        Err(link_refused())
    } else {
        Ok(())
    }
}

fn link_refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Symbolic links, junctions and mount points are not followed on Windows",
    )
}

fn check(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn size_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}
