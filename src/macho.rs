//! Mach-O patching and ad-hoc signing helpers.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::PkgError;

const MACHO_HEADER_SIZE: usize = 32;
const LC_SEGMENT_64: u32 = 0x19;
const LC_SYMTAB: u32 = 0x2;

/// Patch a Mach-O executable after appending pkg payload data.
///
/// The JavaScript producer updates the `__LINKEDIT` segment size and symbol
/// table string size before ad-hoc signing macOS outputs. This function mirrors
/// that byte-level behavior for little-endian 64-bit Mach-O binaries.
///
/// # Example
///
/// ```
/// let mut file = vec![0_u8; 32];
/// file[16..20].copy_from_slice(&0_u32.to_le_bytes());
/// pkg_rust::patch_macho_executable(&mut file)?;
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn patch_macho_executable(file: &mut [u8]) -> Result<(), PkgError> {
    if file.len() < MACHO_HEADER_SIZE {
        return Err(PkgError::Pack(
            "Mach-O file is smaller than the header".to_owned(),
        ));
    }

    let ncmds = read_u32(file, 16)? as usize;
    let mut offset = MACHO_HEADER_SIZE;
    for _index in 0..ncmds {
        let command_start = offset;
        let command_end = command_start
            .checked_add(8)
            .ok_or_else(|| PkgError::Pack("Mach-O load command offset overflowed".to_owned()))?;
        if command_end > file.len() {
            return Err(PkgError::Pack(
                "Mach-O load command is truncated".to_owned(),
            ));
        }

        let command_type = read_u32(file, command_start)?;
        let command_size = read_u32(file, command_start + 4)? as usize;
        if command_size < 8 {
            return Err(PkgError::Pack(
                "Mach-O load command size is invalid".to_owned(),
            ));
        }
        let command_body_start = command_start + 8;
        let command_body_end = command_start
            .checked_add(command_size)
            .ok_or_else(|| PkgError::Pack("Mach-O load command size overflowed".to_owned()))?;
        if command_body_end > file.len() {
            return Err(PkgError::Pack(
                "Mach-O load command body is truncated".to_owned(),
            ));
        }

        patch_command(command_type, command_body_start, command_body_end, file)?;
        offset = command_body_end;
        let padding = offset & 8;
        if padding != 0 {
            offset = offset.checked_add(8 - padding).ok_or_else(|| {
                PkgError::Pack("Mach-O load command alignment overflowed".to_owned())
            })?;
        }
    }

    Ok(())
}

/// Sign a Mach-O executable with an ad-hoc signature.
///
/// This first tries macOS `codesign -f --sign - <path>` and falls back to
/// `ldid -Cadhoc -S <path>`, matching the JavaScript implementation.
///
/// # Example
///
/// ```no_run
/// pkg_rust::sign_macho_executable(std::path::Path::new("./app"))?;
/// # Ok::<(), pkg_rust::PkgError>(())
/// ```
pub fn sign_macho_executable(path: &Path) -> Result<(), PkgError> {
    sign_macho_executable_with_tools(path, Path::new("codesign"), Path::new("ldid"))
}

pub(crate) fn sign_macho_executable_with_tools(
    path: &Path,
    codesign: &Path,
    ldid: &Path,
) -> Result<(), PkgError> {
    match run_codesign(path, codesign) {
        Ok(()) => Ok(()),
        Err(codesign_error) => run_ldid(path, ldid).map_err(|ldid_error| {
            PkgError::Pack(format!(
                "unable to sign Mach-O executable: {codesign_error}; fallback failed: {ldid_error}"
            ))
        }),
    }
}

pub(crate) fn patch_macho_executable_file(path: &Path) -> Result<(), PkgError> {
    let mut file = std::fs::read(path).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })?;
    patch_macho_executable(&mut file)?;
    std::fs::write(path, file).map_err(|source| PkgError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn patch_command(
    command_type: u32,
    body_start: usize,
    body_end: usize,
    file: &mut [u8],
) -> Result<(), PkgError> {
    if command_type == LC_SEGMENT_64 {
        patch_segment_64(body_start, body_end, file)?;
    }
    if command_type == LC_SYMTAB {
        patch_symtab(body_start, body_end, file)?;
    }
    Ok(())
}

fn patch_segment_64(body_start: usize, body_end: usize, file: &mut [u8]) -> Result<(), PkgError> {
    if body_end.saturating_sub(body_start) < 48 {
        return Err(PkgError::Pack(
            "Mach-O segment_64 command is truncated".to_owned(),
        ));
    }
    let name = parse_c_string(&file[body_start..body_start + 16]);
    if name != "__LINKEDIT" {
        return Ok(());
    }

    let fileoff = read_u64(file, body_start + 32)?;
    let file_len = file.len() as u64;
    if fileoff > file_len {
        return Err(PkgError::Pack(
            "Mach-O __LINKEDIT file offset exceeds file size".to_owned(),
        ));
    }
    let patched_size = file_len - fileoff;
    write_u64(file, body_start + 24, patched_size)?;
    write_u64(file, body_start + 40, patched_size)
}

fn patch_symtab(body_start: usize, body_end: usize, file: &mut [u8]) -> Result<(), PkgError> {
    if body_end.saturating_sub(body_start) < 16 {
        return Err(PkgError::Pack(
            "Mach-O symtab command is truncated".to_owned(),
        ));
    }
    let stroff = read_u32(file, body_start + 8)? as usize;
    if stroff > file.len() {
        return Err(PkgError::Pack(
            "Mach-O symtab string offset exceeds file size".to_owned(),
        ));
    }
    let patched_size = file.len() - stroff;
    let patched_size = u32::try_from(patched_size)
        .map_err(|_error| PkgError::Pack("Mach-O symtab string size exceeds u32".to_owned()))?;
    write_u32(file, body_start + 12, patched_size)
}

fn run_codesign(path: &Path, codesign: &Path) -> Result<(), PkgError> {
    let status = Command::new(codesign)
        .arg("-f")
        .arg("--sign")
        .arg("-")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| PkgError::Io {
            path: codesign.display().to_string(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(PkgError::Pack(format!(
            "codesign exited with status {status}"
        )))
    }
}

fn run_ldid(path: &Path, ldid: &Path) -> Result<(), PkgError> {
    let status = Command::new(ldid)
        .arg("-Cadhoc")
        .arg("-S")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| PkgError::Io {
            path: ldid.display().to_string(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(PkgError::Pack(format!("ldid exited with status {status}")))
    }
}

fn parse_c_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PkgError> {
    let range = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| PkgError::Pack("Mach-O u32 read is out of bounds".to_owned()))?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(range);
    Ok(u32::from_le_bytes(value))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), PkgError> {
    let range = bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(|| PkgError::Pack("Mach-O u32 write is out of bounds".to_owned()))?;
    range.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, PkgError> {
    let range = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| PkgError::Pack("Mach-O u64 read is out of bounds".to_owned()))?;
    let mut value = [0_u8; 8];
    value.copy_from_slice(range);
    Ok(u64::from_le_bytes(value))
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), PkgError> {
    let range = bytes
        .get_mut(offset..offset + 8)
        .ok_or_else(|| PkgError::Pack("Mach-O u64 write is out of bounds".to_owned()))?;
    range.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn patch_updates_linkedit_and_symtab_sizes() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = fake_macho();

        patch_macho_executable(&mut file)?;

        assert_eq!(read_test_u64(&file, MACHO_HEADER_SIZE + 8 + 24)?, 48);
        assert_eq!(read_test_u64(&file, MACHO_HEADER_SIZE + 8 + 40)?, 48);
        assert_eq!(read_test_u32(&file, MACHO_HEADER_SIZE + 72 + 8 + 12)?, 38);
        Ok(())
    }

    #[test]
    fn patch_rejects_truncated_files() {
        let mut file = vec![0_u8; 8];

        assert!(patch_macho_executable(&mut file).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn sign_falls_back_to_ldid_when_codesign_fails() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "pkg-rust-macho-sign-fallback-{}",
            std::process::id()
        ));
        let _ignored = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        let executable = temp_dir.join("app");
        fs::write(&executable, b"binary")?;
        let codesign_args = temp_dir.join("codesign.args");
        let ldid_args = temp_dir.join("ldid.args");
        let codesign = script(
            &temp_dir,
            "codesign",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 7\n",
                codesign_args.display()
            ),
        )?;
        let ldid = script(
            &temp_dir,
            "ldid",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n",
                ldid_args.display()
            ),
        )?;

        sign_macho_executable_with_tools(&executable, &codesign, &ldid)?;

        let codesign_args = fs::read_to_string(codesign_args)?;
        let ldid_args = fs::read_to_string(ldid_args)?;
        assert!(codesign_args.contains("-f\n--sign\n-"));
        assert!(ldid_args.contains("-Cadhoc\n-S"));
        assert!(ldid_args.contains(&executable.to_string_lossy().into_owned()));

        fs::remove_dir_all(temp_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    fn script(
        temp_dir: &Path,
        name: &str,
        body: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.join(name);
        fs::write(&path, body)?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(path)
    }

    fn fake_macho() -> Vec<u8> {
        let mut file = vec![0_u8; 128];
        write_test_u32(&mut file, 16, 2);

        let segment = MACHO_HEADER_SIZE;
        write_test_u32(&mut file, segment, LC_SEGMENT_64);
        write_test_u32(&mut file, segment + 4, 72);
        file[segment + 8..segment + 18].copy_from_slice(b"__LINKEDIT");
        write_test_u64(&mut file, segment + 8 + 24, 1);
        write_test_u64(&mut file, segment + 8 + 32, 80);
        write_test_u64(&mut file, segment + 8 + 40, 2);

        let symtab = MACHO_HEADER_SIZE + 72;
        write_test_u32(&mut file, symtab, LC_SYMTAB);
        write_test_u32(&mut file, symtab + 4, 24);
        write_test_u32(&mut file, symtab + 8 + 8, 90);
        write_test_u32(&mut file, symtab + 8 + 12, 3);
        file
    }

    fn read_test_u32(bytes: &[u8], offset: usize) -> Result<u32, PkgError> {
        let range = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| PkgError::Pack("test u32 read out of bounds".to_owned()))?;
        let mut value = [0_u8; 4];
        value.copy_from_slice(range);
        Ok(u32::from_le_bytes(value))
    }

    fn read_test_u64(bytes: &[u8], offset: usize) -> Result<u64, PkgError> {
        let range = bytes
            .get(offset..offset + 8)
            .ok_or_else(|| PkgError::Pack("test u64 read out of bounds".to_owned()))?;
        let mut value = [0_u8; 8];
        value.copy_from_slice(range);
        Ok(u64::from_le_bytes(value))
    }

    fn write_test_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_test_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
