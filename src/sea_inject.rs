//! Native injection of the `NODE_SEA_BLOB` resource and SEA fuse flip.
//!
//! yao-pkg/pkg shells out to the LIEF-based `postject` library to add the
//! `NODE_SEA_BLOB` section/segment/resource and flip the SEA fuse sentinel.
//! This port performs the injection natively so the produced-binary path stays
//! entirely in Rust with no npm/build-time tool boundary.
//!
//! # How Node finds the blob
//!
//! Node's runtime resource finder ([`postject-api.h`]) locates the blob
//! differently per object format:
//!
//! - **ELF**: it does **not** look up a section by name. It walks the loaded
//!   program headers via `dl_iterate_phdr`, scans every `PT_NOTE` segment, and
//!   returns the descriptor of the first note whose name matches
//!   `NODE_SEA_BLOB`. So injection appends the blob as an ELF note, maps it with
//!   a fresh `PT_LOAD`, and exposes it through a fresh `PT_NOTE` program header.
//! - **Mach-O**: `getsectdata("NODE_SEA", "__NODE_SEA_BLOB")` — a dedicated
//!   segment/section.
//! - **PE**: `FindResourceA(RT_RCDATA, "NODE_SEA_BLOB")` — an `RT_RCDATA`
//!   resource.
//!
//! The fuse is the string `NODE_SEA_FUSE_<hash>:0`; Node treats the blob as
//! present only when the trailing byte is `1` (see `postject_has_resource`), so
//! injection flips that `0` to `1`.
//!
//! [`postject-api.h`]: https://github.com/nodejs/postject/blob/main/postject-api.h
//!
//! # Verification status
//!
//! ELF injection is verified end to end against the real Node 22 runtime on
//! Linux x64 (the produced executable runs the embedded SEA main). Mach-O and PE
//! injection are not yet implemented natively and fail closed with a precise
//! error rather than emitting a binary that would crash at startup.

use crate::error::PkgError;
use crate::target::Platform;

/// The ELF note name Node's resource finder matches (NUL-terminated).
const NOTE_NAME: &[u8] = b"NODE_SEA_BLOB\0";

/// Build the SEA fuse sentinel by concatenation.
///
/// Like yao-pkg, the literal is assembled at runtime so the full sentinel never
/// appears as a single contiguous string inside this packager's own binary. If
/// `@yao-pkg/pkg` (or this port) were itself packaged into a SEA archive, a
/// verbatim sentinel in the injector would collide with the one in the target
/// Node binary and make the fuse search ambiguous.
fn sea_fuse_sentinel() -> String {
    let mut sentinel = String::from("NODE_SEA");
    sentinel.push_str("_FUSE_fce680ab2cc467b6e072b8b5df1996b2");
    sentinel
}

/// Whether native `NODE_SEA_BLOB` injection is implemented for `platform`.
///
/// Only ELF (Linux-family) targets are supported today; macOS (Mach-O) and
/// Windows (PE) injection are not implemented yet. Callers gate on this before
/// downloading binaries so an unsupported target fails before any work, rather
/// than after generating the blob (see [`inject_sea_blob`]).
pub(crate) fn injection_supported(platform: Platform) -> bool {
    matches!(
        platform,
        Platform::Linux | Platform::LinuxStatic | Platform::Alpine | Platform::Freebsd
    )
}

/// Flip the fuse in the target executable image and inject the SEA blob.
///
/// `platform` selects the object-format strategy; the magic bytes are validated
/// against it so a mismatched binary fails loudly instead of being corrupted.
pub(crate) fn inject_sea_blob(
    mut image: Vec<u8>,
    blob: &[u8],
    platform: Platform,
) -> Result<Vec<u8>, PkgError> {
    match platform {
        Platform::Win => inject_pe(image, blob),
        Platform::Macos => inject_macho(image, blob),
        Platform::Linux | Platform::LinuxStatic | Platform::Alpine | Platform::Freebsd => {
            flip_sea_fuse(&mut image)?;
            inject_elf(image, blob)
        }
    }
}

/// Flip the SEA fuse sentinel `…:0` to `…:1` so Node activates the blob.
///
/// Mirrors postject's fuse handling: the sentinel must occur exactly once in its
/// inactive (`:0`) form. Zero occurrences means the binary is not a SEA-capable
/// Node build (or was already injected); multiple occurrences are ambiguous.
fn flip_sea_fuse(image: &mut [u8]) -> Result<(), PkgError> {
    let mut needle = sea_fuse_sentinel().into_bytes();
    needle.extend_from_slice(b":0");

    let mut matches = image
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle.as_slice())
        .map(|(index, _)| index);

    let Some(start) = matches.next() else {
        return Err(PkgError::Sea(
            "SEA fuse sentinel not found in the Node binary; the downloaded base \
             binary is not a SEA-capable build"
                .to_owned(),
        ));
    };
    if matches.next().is_some() {
        return Err(PkgError::Sea(
            "Multiple occurrences of the SEA fuse sentinel in the Node binary".to_owned(),
        ));
    }

    // `start + needle.len() - 1` is the trailing `0` byte; flip it to `1`.
    let fuse_index = start + needle.len() - 1;
    image[fuse_index] = b'1';
    Ok(())
}

/// Round `value` up to the next multiple of `align` (a power of two).
fn roundup(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (value + (align - 1)) & !(align - 1)
}

/// Read/write multi-byte integers honoring an ELF file's endianness.
#[derive(Clone, Copy)]
struct Endian {
    little: bool,
}

impl Endian {
    fn read_u16(self, bytes: &[u8]) -> u16 {
        let array = [bytes[0], bytes[1]];
        if self.little {
            u16::from_le_bytes(array)
        } else {
            u16::from_be_bytes(array)
        }
    }

    fn read_u32(self, bytes: &[u8]) -> u32 {
        let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if self.little {
            u32::from_le_bytes(array)
        } else {
            u32::from_be_bytes(array)
        }
    }

    fn read_u64(self, bytes: &[u8]) -> u64 {
        let mut array = [0_u8; 8];
        array.copy_from_slice(&bytes[..8]);
        if self.little {
            u64::from_le_bytes(array)
        } else {
            u64::from_be_bytes(array)
        }
    }

    fn write_u16(self, out: &mut [u8], value: u16) {
        let bytes = if self.little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        out[..2].copy_from_slice(&bytes);
    }

    fn write_u32(self, out: &mut Vec<u8>, value: u32) {
        if self.little {
            out.extend_from_slice(&value.to_le_bytes());
        } else {
            out.extend_from_slice(&value.to_be_bytes());
        }
    }

    fn write_u64_at(self, out: &mut [u8], value: u64) {
        let bytes = if self.little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        out[..8].copy_from_slice(&bytes);
    }

    fn push_u64(self, out: &mut Vec<u8>, value: u64) {
        if self.little {
            out.extend_from_slice(&value.to_le_bytes());
        } else {
            out.extend_from_slice(&value.to_be_bytes());
        }
    }
}

/// ELF64 program header (decoded), preserving field order for re-emission.
#[derive(Clone)]
struct Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;
const PT_NOTE: u32 = 4;
const PT_PHDR: u32 = 6;
const PF_R: u32 = 4;
const PHDR64_SIZE: usize = 56;

/// ELF header field offsets (ELF64).
const E_TYPE: usize = 16;
const E_PHOFF: usize = 32;
const E_PHENTSIZE: usize = 54;
const E_PHNUM: usize = 56;

/// Inject the blob as a `PT_NOTE`-mapped ELF note named `NODE_SEA_BLOB`.
///
/// Algorithm (verified against the Node 22 SEA runtime on Linux x64):
///   1. Build an ELF note `{ namesz, descsz, type=0 } name="NODE_SEA_BLOB\0"
///      desc=blob`, each field padded to 4 bytes.
///   2. Append, page-aligned at end of file: the note, then a relocated copy of
///      the program header table with two extra entries.
///   3. Repoint the existing `PT_PHDR` entry at the relocated table (glibc
///      derives the load bias as `AT_PHDR − PT_PHDR.p_vaddr`, so a stale
///      `PT_PHDR` yields a garbage bias and an early `ld.so` crash).
///   4. Add a read-only `PT_LOAD` covering the appended region and a `PT_NOTE`
///      pointing at the note.
///   5. Update `e_phoff` / `e_phnum`.
fn inject_elf(mut image: Vec<u8>, blob: &[u8]) -> Result<Vec<u8>, PkgError> {
    if image.len() < 64 || image.get(..4) != Some(b"\x7fELF") {
        return Err(PkgError::Sea("not an ELF binary".to_owned()));
    }
    if image[4] != 2 {
        return Err(PkgError::Sea(
            "native SEA injection supports 64-bit ELF only; 32-bit targets \
             (x86/armv7) are not yet supported"
                .to_owned(),
        ));
    }
    let endian = Endian {
        little: image[5] == 1,
    };

    let e_phoff = endian.read_u64(&image[E_PHOFF..]) as usize;
    let e_phentsize = endian.read_u16(&image[E_PHENTSIZE..]) as usize;
    let e_phnum = endian.read_u16(&image[E_PHNUM..]) as usize;
    if e_phentsize != PHDR64_SIZE {
        return Err(PkgError::Sea(format!(
            "unexpected ELF program-header entry size {e_phentsize} (expected {PHDR64_SIZE})"
        )));
    }
    let phdr_table_end = e_phoff.saturating_add(e_phnum.saturating_mul(e_phentsize));
    if phdr_table_end > image.len() {
        return Err(PkgError::Sea(
            "ELF program header table extends past end of file".to_owned(),
        ));
    }

    let mut phdrs = Vec::with_capacity(e_phnum);
    let mut max_vaddr_end = 0_u64;
    for index in 0..e_phnum {
        let base = e_phoff + index * e_phentsize;
        let phdr = Phdr {
            p_type: endian.read_u32(&image[base..]),
            p_flags: endian.read_u32(&image[base + 4..]),
            p_offset: endian.read_u64(&image[base + 8..]),
            p_vaddr: endian.read_u64(&image[base + 16..]),
            p_paddr: endian.read_u64(&image[base + 24..]),
            p_filesz: endian.read_u64(&image[base + 32..]),
            p_memsz: endian.read_u64(&image[base + 40..]),
            p_align: endian.read_u64(&image[base + 48..]),
        };
        if phdr.p_type == PT_LOAD {
            max_vaddr_end = max_vaddr_end.max(phdr.p_vaddr.saturating_add(phdr.p_memsz));
        }
        phdrs.push(phdr);
    }

    // 1. Build the note.
    let note = build_elf_note(endian, blob);

    // 2. Lay out the appended region.
    let page = 0x1000_u64;
    let seg_offset = roundup(image.len() as u64, page);
    let note_offset = seg_offset;
    let new_phoff = roundup(note_offset + note.len() as u64, 8);
    let new_phnum = e_phnum + 2;
    let seg_end = new_phoff + (new_phnum as u64) * (PHDR64_SIZE as u64);
    let seg_filesz = seg_end - seg_offset;
    // Pick a fresh load address above every existing segment, congruent to the
    // file offset modulo the page size (mmap requires `vaddr ≡ offset mod align`).
    let seg_vaddr = roundup(max_vaddr_end + page, page) + (seg_offset % page);
    let note_vaddr = seg_vaddr + (note_offset - seg_offset);
    let phdr_vaddr = seg_vaddr + (new_phoff - seg_offset);

    // 3. Repoint PT_PHDR (if present) at the relocated table.
    for phdr in &mut phdrs {
        if phdr.p_type == PT_PHDR {
            phdr.p_offset = new_phoff;
            phdr.p_vaddr = phdr_vaddr;
            phdr.p_paddr = phdr_vaddr;
            phdr.p_filesz = (new_phnum as u64) * (PHDR64_SIZE as u64);
            phdr.p_memsz = phdr.p_filesz;
        }
    }

    // 4. Append the two new program headers.
    phdrs.push(Phdr {
        p_type: PT_LOAD,
        p_flags: PF_R,
        p_offset: seg_offset,
        p_vaddr: seg_vaddr,
        p_paddr: seg_vaddr,
        p_filesz: seg_filesz,
        p_memsz: seg_filesz,
        p_align: page,
    });
    phdrs.push(Phdr {
        p_type: PT_NOTE,
        p_flags: PF_R,
        p_offset: note_offset,
        p_vaddr: note_vaddr,
        p_paddr: note_vaddr,
        p_filesz: note.len() as u64,
        p_memsz: note.len() as u64,
        p_align: 4,
    });

    // Write the appended region: pad to seg_offset, note, pad to new_phoff, table.
    image.resize(seg_offset as usize, 0);
    image.extend_from_slice(&note);
    image.resize(new_phoff as usize, 0);
    for phdr in &phdrs {
        endian.write_u32(&mut image, phdr.p_type);
        endian.write_u32(&mut image, phdr.p_flags);
        endian.push_u64(&mut image, phdr.p_offset);
        endian.push_u64(&mut image, phdr.p_vaddr);
        endian.push_u64(&mut image, phdr.p_paddr);
        endian.push_u64(&mut image, phdr.p_filesz);
        endian.push_u64(&mut image, phdr.p_memsz);
        endian.push_u64(&mut image, phdr.p_align);
    }

    // 5. Patch the ELF header.
    endian.write_u64_at(&mut image[E_PHOFF..], new_phoff);
    endian.write_u16(&mut image[E_PHNUM..], new_phnum as u16);

    // ET_EXEC binaries keep absolute addresses; nothing else needs touching. The
    // caller flips the SEA fuse before appending the blob so payload bytes cannot
    // create false duplicate fuse matches.
    let _ = E_TYPE;

    Ok(image)
}

/// Build an ELF note `{namesz, descsz, type=0}` + padded name + padded desc.
fn build_elf_note(endian: Endian, blob: &[u8]) -> Vec<u8> {
    let name_padded = roundup(NOTE_NAME.len() as u64, 4) as usize;
    let desc_padded = roundup(blob.len() as u64, 4) as usize;
    let mut note = Vec::with_capacity(12 + name_padded + desc_padded);
    endian.write_u32(&mut note, NOTE_NAME.len() as u32);
    endian.write_u32(&mut note, blob.len() as u32);
    endian.write_u32(&mut note, 0); // n_type (ignored by Node's finder)
    note.extend_from_slice(NOTE_NAME);
    note.resize(12 + name_padded, 0);
    note.extend_from_slice(blob);
    note.resize(12 + name_padded + desc_padded, 0);
    note
}

/// Mach-O injection is not yet implemented natively.
fn inject_macho(_image: Vec<u8>, _blob: &[u8]) -> Result<Vec<u8>, PkgError> {
    Err(PkgError::Sea(
        "native SEA injection for macOS (Mach-O) targets is not implemented yet; \
         build SEA executables for Linux targets, or target macOS in a later slice"
            .to_owned(),
    ))
}

/// PE injection is not yet implemented natively.
fn inject_pe(_image: Vec<u8>, _blob: &[u8]) -> Result<Vec<u8>, PkgError> {
    Err(PkgError::Sea(
        "native SEA injection for Windows (PE) targets is not implemented yet; \
         build SEA executables for Linux targets, or target Windows in a later slice"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal ELF64 LE image with an ELF header + one PT_PHDR + one PT_LOAD,
    /// plus a fuse sentinel, used to exercise the note/phdr surgery offline.
    fn synthetic_elf(fuse_count: usize) -> Vec<u8> {
        let endian = Endian { little: true };
        let phnum = 2_u32;
        let e_phoff = 64_u64;
        let mut image = vec![0_u8; 64 + (phnum as usize) * PHDR64_SIZE];
        image[..4].copy_from_slice(b"\x7fELF");
        image[4] = 2; // 64-bit
        image[5] = 1; // little endian
        endian.write_u16(&mut image[E_TYPE..], 2); // ET_EXEC
        endian.write_u64_at(&mut image[E_PHOFF..], e_phoff);
        endian.write_u16(&mut image[E_PHENTSIZE..], PHDR64_SIZE as u16);
        endian.write_u16(&mut image[E_PHNUM..], phnum as u16);

        let write_phdr = |image: &mut [u8], index: usize, phdr: &Phdr| {
            let base = e_phoff as usize + index * PHDR64_SIZE;
            endian.write_u32_at(&mut image[base..], phdr.p_type);
            endian.write_u32_at(&mut image[base + 4..], phdr.p_flags);
            endian.write_u64_at(&mut image[base + 8..], phdr.p_offset);
            endian.write_u64_at(&mut image[base + 16..], phdr.p_vaddr);
            endian.write_u64_at(&mut image[base + 24..], phdr.p_paddr);
            endian.write_u64_at(&mut image[base + 32..], phdr.p_filesz);
            endian.write_u64_at(&mut image[base + 40..], phdr.p_memsz);
            endian.write_u64_at(&mut image[base + 48..], phdr.p_align);
        };
        write_phdr(
            &mut image,
            0,
            &Phdr {
                p_type: PT_PHDR,
                p_flags: PF_R,
                p_offset: e_phoff,
                p_vaddr: 0x40_0040,
                p_paddr: 0x40_0040,
                p_filesz: (phnum as u64) * (PHDR64_SIZE as u64),
                p_memsz: (phnum as u64) * (PHDR64_SIZE as u64),
                p_align: 8,
            },
        );
        write_phdr(
            &mut image,
            1,
            &Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R,
                p_offset: 0,
                p_vaddr: 0x40_0000,
                p_paddr: 0x40_0000,
                p_filesz: 0x1000,
                p_memsz: 0x1000,
                p_align: 0x1000,
            },
        );

        for _ in 0..fuse_count {
            image.extend_from_slice(format!("{}:0", sea_fuse_sentinel()).as_bytes());
        }
        image
    }

    impl Endian {
        fn write_u32_at(self, out: &mut [u8], value: u32) {
            let bytes = if self.little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            };
            out[..4].copy_from_slice(&bytes);
        }
    }

    fn read_header(image: &[u8]) -> (Endian, u64, usize, usize) {
        let endian = Endian {
            little: image[5] == 1,
        };
        let e_phoff = endian.read_u64(&image[E_PHOFF..]);
        let e_phentsize = endian.read_u16(&image[E_PHENTSIZE..]) as usize;
        let e_phnum = endian.read_u16(&image[E_PHNUM..]) as usize;
        (endian, e_phoff, e_phentsize, e_phnum)
    }

    #[test]
    fn elf_injection_adds_load_and_note_segments() -> Result<(), PkgError> {
        let blob = b"single-executable-blob-payload".to_vec();
        let image = synthetic_elf(1);
        let (_, _, _, original_phnum) = read_header(&image);

        let injected = inject_sea_blob(image, &blob, Platform::Linux)?;
        let (endian, e_phoff, e_phentsize, e_phnum) = read_header(&injected);
        assert_eq!(e_phnum, original_phnum + 2, "two program headers added");

        // Collect the new program headers and locate the PT_NOTE we added.
        let mut saw_load = false;
        let mut note_segment: Option<Phdr> = None;
        let mut phdr_segment: Option<Phdr> = None;
        for index in 0..e_phnum {
            let base = e_phoff as usize + index * e_phentsize;
            let phdr = Phdr {
                p_type: endian.read_u32(&injected[base..]),
                p_flags: endian.read_u32(&injected[base + 4..]),
                p_offset: endian.read_u64(&injected[base + 8..]),
                p_vaddr: endian.read_u64(&injected[base + 16..]),
                p_paddr: endian.read_u64(&injected[base + 24..]),
                p_filesz: endian.read_u64(&injected[base + 32..]),
                p_memsz: endian.read_u64(&injected[base + 40..]),
                p_align: endian.read_u64(&injected[base + 48..]),
            };
            match phdr.p_type {
                PT_LOAD => saw_load = true,
                PT_NOTE => note_segment = Some(phdr.clone()),
                PT_PHDR => phdr_segment = Some(phdr.clone()),
                _ => {}
            }
        }
        assert!(saw_load, "a PT_LOAD segment is present");

        // PT_PHDR must be repointed at the relocated table.
        let Some(phdr_segment) = phdr_segment else {
            return Err(PkgError::Sea("PT_PHDR missing after injection".to_owned()));
        };
        assert_eq!(phdr_segment.p_offset, e_phoff, "PT_PHDR repointed to table");
        assert_eq!(
            phdr_segment.p_filesz,
            (e_phnum as u64) * (e_phentsize as u64),
            "PT_PHDR covers all entries",
        );

        // The note descriptor must contain the blob verbatim.
        let Some(note) = note_segment else {
            return Err(PkgError::Sea("PT_NOTE missing after injection".to_owned()));
        };
        let note_start = note.p_offset as usize;
        let namesz = endian.read_u32(&injected[note_start..]) as usize;
        let descsz = endian.read_u32(&injected[note_start + 4..]) as usize;
        assert_eq!(namesz, NOTE_NAME.len());
        assert_eq!(descsz, blob.len());
        let name_start = note_start + 12;
        assert_eq!(&injected[name_start..name_start + namesz], NOTE_NAME);
        let desc_start = name_start + roundup(namesz as u64, 4) as usize;
        assert_eq!(&injected[desc_start..desc_start + descsz], blob.as_slice());
        Ok(())
    }

    #[test]
    fn flips_the_fuse_exactly_once() -> Result<(), PkgError> {
        let image = synthetic_elf(1);
        let injected = inject_sea_blob(image, b"blob", Platform::Linux)?;
        let active = format!("{}:1", sea_fuse_sentinel());
        let inactive = format!("{}:0", sea_fuse_sentinel());
        let count_active = injected
            .windows(active.len())
            .filter(|window| *window == active.as_bytes())
            .count();
        assert_eq!(count_active, 1, "fuse activated");
        let count_inactive = injected
            .windows(inactive.len())
            .filter(|window| *window == inactive.as_bytes())
            .count();
        assert_eq!(count_inactive, 0, "no inactive fuse remains");
        Ok(())
    }

    #[test]
    fn inactive_fuse_text_inside_blob_does_not_look_like_duplicate_binary_fuse()
    -> Result<(), PkgError> {
        let inactive = format!("{}:0", sea_fuse_sentinel());
        let active = format!("{}:1", sea_fuse_sentinel());
        let blob = format!("test fixture includes {inactive} as plain payload text");

        let injected = inject_sea_blob(synthetic_elf(1), blob.as_bytes(), Platform::Linux)?;

        let count_active = injected
            .windows(active.len())
            .filter(|window| *window == active.as_bytes())
            .count();
        assert_eq!(count_active, 1, "base binary fuse activated once");

        let count_inactive = injected
            .windows(inactive.len())
            .filter(|window| *window == inactive.as_bytes())
            .count();
        assert_eq!(
            count_inactive, 1,
            "inactive marker inside the SEA blob stays payload data"
        );
        Ok(())
    }

    #[test]
    fn missing_fuse_is_rejected() {
        assert!(matches!(
            inject_sea_blob(synthetic_elf(0), b"blob", Platform::Linux),
            Err(PkgError::Sea(message)) if message.contains("fuse sentinel not found")
        ));
    }

    #[test]
    fn duplicate_fuse_is_rejected() {
        assert!(matches!(
            inject_sea_blob(synthetic_elf(2), b"blob", Platform::Linux),
            Err(PkgError::Sea(message)) if message.contains("Multiple occurrences")
        ));
    }

    #[test]
    fn macos_and_windows_fail_closed() {
        assert!(matches!(
            inject_sea_blob(synthetic_elf(1), b"blob", Platform::Macos),
            Err(PkgError::Sea(message)) if message.contains("macOS")
        ));
        assert!(matches!(
            inject_sea_blob(synthetic_elf(1), b"blob", Platform::Win),
            Err(PkgError::Sea(message)) if message.contains("Windows")
        ));
    }

    #[test]
    fn unsupported_formats_fail_before_fuse_scanning() {
        assert!(matches!(
            inject_sea_blob(synthetic_elf(0), b"blob", Platform::Macos),
            Err(PkgError::Sea(message)) if message.contains("macOS")
        ));
        assert!(matches!(
            inject_sea_blob(synthetic_elf(0), b"blob", Platform::Win),
            Err(PkgError::Sea(message)) if message.contains("Windows")
        ));
    }

    /// End-to-end verification of native ELF injection against the real Node
    /// runtime: generate a SEA blob with host `node --experimental-sea-config`,
    /// inject it into a copy of that same `node` with [`inject_sea_blob`], run the
    /// result, and assert the embedded main executed.
    ///
    /// Gated on a SEA-capable Linux host (Node >= 22, ELF). Skips gracefully
    /// elsewhere so `cargo test` stays green in environments without Node.
    #[test]
    fn elf_injection_runs_against_real_node_runtime() -> Result<(), Box<dyn std::error::Error>> {
        use std::process::Command;

        if !cfg!(target_os = "linux") || std::env::consts::ARCH != "x86_64" {
            eprintln!("skipping SEA ELF runtime smoke: not a linux-x64 host");
            return Ok(());
        }
        let Some(node) = locate_host_node_ge_22() else {
            eprintln!("skipping SEA ELF runtime smoke: no Node >= 22 on PATH");
            return Ok(());
        };

        let dir = std::env::temp_dir().join(format!(
            "pkg-sea-inject-smoke-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&dir)?;

        let entry = dir.join("entry.js");
        std::fs::write(
            &entry,
            "console.log('PKG_SEA_RUST_INJECT_OK ' + process.argv0);",
        )?;
        let blob_path = dir.join("sea-prep.blob");
        let config = dir.join("sea-config.json");
        let config_json = serde_json::json!({
            "main": entry,
            "output": blob_path,
            "disableExperimentalSEAWarning": true,
            "useSnapshot": false,
            "useCodeCache": false,
        });
        std::fs::write(&config, serde_json::to_vec(&config_json)?)?;

        let status = Command::new(&node)
            .arg("--experimental-sea-config")
            .arg(&config)
            .output()?;
        assert!(
            status.status.success(),
            "blob generation failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let blob = std::fs::read(&blob_path)?;

        let node_bytes = std::fs::read(&node)?;
        let injected = inject_sea_blob(node_bytes, &blob, Platform::Linux)?;
        let output = dir.join("sea-app");
        std::fs::write(&output, &injected)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o755))?;
        }

        let run = Command::new(&output).output()?;
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            run.status.success() && stdout.contains("PKG_SEA_RUST_INJECT_OK"),
            "injected SEA binary did not run the embedded main\nstdout: {stdout}\nstderr: {stderr}"
        );
        Ok(())
    }

    /// Find a Node >= 22 on `PATH` (or `$PKG_SEA_TEST_NODE`) for the smoke test.
    fn locate_host_node_ge_22() -> Option<std::path::PathBuf> {
        use std::process::Command;
        let candidate = std::env::var_os("PKG_SEA_TEST_NODE")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let paths = std::env::var_os("PATH")?;
                std::env::split_paths(&paths)
                    .map(|dir| dir.join("node"))
                    .find(|candidate| candidate.is_file())
            })?;
        let version = Command::new(&candidate).arg("--version").output().ok()?;
        if !version.status.success() {
            return None;
        }
        let text = String::from_utf8(version.stdout).ok()?;
        let major: u32 = text
            .trim()
            .trim_start_matches('v')
            .split('.')
            .next()?
            .parse()
            .ok()?;
        (major >= 22).then_some(candidate)
    }
}
