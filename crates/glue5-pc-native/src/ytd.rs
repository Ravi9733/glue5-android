//! Bounded metadata parser for GTA V Legacy YTD texture dictionaries.
//!
//! This parses the RSC7 page layout and texture descriptors. It validates
//! virtual-pointer ranges but deliberately does not decode BC/DDS pixel formats.

use crate::{
    read_rsc7_pointer_from_pages, resolve_rsc7_pointer, ManifestError,
    Rpf7ResourcePageSizes, Rsc7ResolvedPointer,
};

const MAX_TEXTURES: usize = 4096;
const MAX_NAME_BYTES: u32 = 256;
const DICTIONARY_HEADER_BYTES: u32 = 0x40;
const TEXTURE_HEADER_BYTES: u32 = 0x90;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YtdTextureInfo {
    pub name: String,
    pub name_hash: u32,
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub stride: u16,
    pub format_code: u32,
    pub mip_levels: u8,
    pub pixel_data: Option<Rsc7ResolvedPointer>,
    pub pixel_data_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YtdDictionary {
    pub textures: Vec<YtdTextureInfo>,
}

/// Parse a GTA V Legacy YTD dictionary from already-decompressed RSC7 pages.
/// The dictionary is rooted at system virtual address 0x50000000.
pub fn parse_ytd_resource_pages(
    system: &[u8],
    graphics: &[u8],
    page_sizes: Rpf7ResourcePageSizes,
) -> Result<YtdDictionary, ManifestError> {
    validate_pages(system, graphics, page_sizes)?;
    let root = read_rsc7_pointer_from_pages(
        system, graphics, 0x5000_0000, DICTIONARY_HEADER_BYTES, page_sizes,
    )?.ok_or_else(|| ManifestError("YTD dictionary root is null".into()))?;

    let hash_pointer = read_u64(root, 0x20)?;
    let hash_count = usize::from(read_u16(root, 0x28)?);
    let texture_pointer = read_u64(root, 0x30)?;
    let texture_count = usize::from(read_u16(root, 0x38)?);
    let texture_capacity = usize::from(read_u16(root, 0x3A)?);

    if texture_count > texture_capacity || texture_count > MAX_TEXTURES {
        return Err(ManifestError(format!(
            "YTD texture count {texture_count} exceeds capacity {texture_capacity} or safety limit {MAX_TEXTURES}"
        )));
    }
    if hash_count > MAX_TEXTURES {
        return Err(ManifestError("YTD name-hash count exceeds safety limit".into()));
    }

    let hashes = if hash_count == 0 {
        Vec::new()
    } else {
        let byte_len = u32::try_from(hash_count.checked_mul(4)
            .ok_or_else(|| ManifestError("YTD hash array length overflow".into()))?)
            .map_err(|_| ManifestError("YTD hash array is too large".into()))?;
        let bytes = read_rsc7_pointer_from_pages(
            system, graphics, hash_pointer, byte_len, page_sizes,
        )?.ok_or_else(|| ManifestError("YTD hash array pointer is null".into()))?;
        bytes.chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>()
    };

    if texture_count == 0 {
        return Ok(YtdDictionary { textures: Vec::new() });
    }
    let pointer_bytes = u32::try_from(texture_count.checked_mul(8)
        .ok_or_else(|| ManifestError("YTD texture pointer array length overflow".into()))?)
        .map_err(|_| ManifestError("YTD texture pointer array is too large".into()))?;
    let pointer_data = read_rsc7_pointer_from_pages(
        system, graphics, texture_pointer, pointer_bytes, page_sizes,
    )?.ok_or_else(|| ManifestError("YTD texture pointer array is null".into()))?;

    let mut textures = Vec::with_capacity(texture_count);
    for (index, raw_pointer) in pointer_data.chunks_exact(8).enumerate() {
        let texture_va = u64::from_le_bytes(raw_pointer.try_into().unwrap());
        if texture_va == 0 {
            continue;
        }
        let raw = read_rsc7_pointer_from_pages(
            system, graphics, texture_va, TEXTURE_HEADER_BYTES, page_sizes,
        )?.ok_or_else(|| ManifestError(format!("YTD texture {index} has a null pointer")))?;

        let name_pointer = read_u64(raw, 0x28)?;
        let width = read_u16(raw, 0x50)?;
        let height = read_u16(raw, 0x52)?;
        let depth = read_u16(raw, 0x54)?;
        let stride = read_u16(raw, 0x56)?;
        let format_code = read_u32(raw, 0x58)?;
        let mip_levels = *raw.get(0x5D)
            .ok_or_else(|| ManifestError("YTD texture header is truncated".into()))?;
        let data_pointer = read_u64(raw, 0x70)?;

        if width == 0 || height == 0 || stride == 0 || mip_levels == 0 {
            return Err(ManifestError(format!("YTD texture {index} has invalid dimensions, stride, or mip count")));
        }
        let pixel_data_bytes = mip_chain_bytes(stride, height, mip_levels)?;
        let pixel_data = if pixel_data_bytes == 0 {
            None
        } else {
            Some(resolve_rsc7_pointer(data_pointer, pixel_data_bytes, page_sizes)?
                .ok_or_else(|| ManifestError(format!("YTD texture {index} has a null pixel-data pointer")))?)
        };

        textures.push(YtdTextureInfo {
            name: read_rsc7_string(system, graphics, name_pointer, page_sizes)?,
            name_hash: hashes.get(index).copied().unwrap_or(0),
            width,
            height,
            depth,
            stride,
            format_code,
            mip_levels,
            pixel_data,
            pixel_data_bytes,
        });
    }

    Ok(YtdDictionary { textures })
}

fn validate_pages(
    system: &[u8],
    graphics: &[u8],
    sizes: Rpf7ResourcePageSizes,
) -> Result<(), ManifestError> {
    if u64::try_from(system.len()).ok() != Some(sizes.system_bytes)
        || u64::try_from(graphics.len()).ok() != Some(sizes.graphics_bytes)
    {
        return Err(ManifestError("YTD page buffers do not match declared RSC7 page sizes".into()));
    }
    Ok(())
}

fn read_rsc7_string(
    system: &[u8],
    graphics: &[u8],
    pointer: u64,
    sizes: Rpf7ResourcePageSizes,
) -> Result<String, ManifestError> {
    if pointer == 0 {
        return Ok(String::new());
    }
    let mut bytes = Vec::new();
    for index in 0..MAX_NAME_BYTES {
        let address = pointer.checked_add(u64::from(index))
            .ok_or_else(|| ManifestError("YTD name pointer overflow".into()))?;
        let byte = read_rsc7_pointer_from_pages(system, graphics, address, 1, sizes)?
            .ok_or_else(|| ManifestError("YTD name pointer resolved to null".into()))?[0];
        if byte == 0 {
            return String::from_utf8(bytes)
                .map_err(|_| ManifestError("YTD texture name is not valid UTF-8".into()));
        }
        bytes.push(byte);
    }
    Err(ManifestError("YTD texture name exceeds maximum length".into()))
}

fn mip_chain_bytes(stride: u16, height: u16, levels: u8) -> Result<u32, ManifestError> {
    let mut level_bytes = u64::from(stride) * u64::from(height);
    let mut total = 0_u64;
    for _ in 0..levels {
        total = total.checked_add(level_bytes)
            .ok_or_else(|| ManifestError("YTD mip-chain size overflow".into()))?;
        level_bytes /= 4;
    }
    u32::try_from(total).map_err(|_| ManifestError("YTD mip-chain exceeds supported pointer range".into()))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ManifestError> {
    let slice = bytes.get(offset..offset + 2)
        .ok_or_else(|| ManifestError("YTD structure is truncated reading u16".into()))?;
    Ok(u16::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ManifestError> {
    let slice = bytes.get(offset..offset + 4)
        .ok_or_else(|| ManifestError("YTD structure is truncated reading u32".into()))?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ManifestError> {
    let slice = bytes.get(offset..offset + 8)
        .ok_or_else(|| ManifestError("YTD structure is truncated reading u64".into()))?;
    Ok(u64::from_le_bytes(slice.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_texture_dictionary_and_validates_graphics_pointer() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 512, graphics_bytes: 64 };
        let mut system = vec![0_u8; 512];
        let graphics = vec![0xAB; 64];

        // Dictionary at system base: one hash and one texture pointer.
        system[0x20..0x28].copy_from_slice(&0x5000_0040_u64.to_le_bytes());
        system[0x28..0x2A].copy_from_slice(&1_u16.to_le_bytes());
        system[0x30..0x38].copy_from_slice(&0x5000_0048_u64.to_le_bytes());
        system[0x38..0x3A].copy_from_slice(&1_u16.to_le_bytes());
        system[0x3A..0x3C].copy_from_slice(&1_u16.to_le_bytes());
        system[0x40..0x44].copy_from_slice(&0x1234_ABCD_u32.to_le_bytes());
        system[0x48..0x50].copy_from_slice(&0x5000_0080_u64.to_le_bytes());

        // Texture descriptor and its name.
        system[0xA8..0xB0].copy_from_slice(&0x5000_0100_u64.to_le_bytes());
        system[0xD0..0xD2].copy_from_slice(&4_u16.to_le_bytes());
        system[0xD2..0xD4].copy_from_slice(&4_u16.to_le_bytes());
        system[0xD4..0xD6].copy_from_slice(&1_u16.to_le_bytes());
        system[0xD6..0xD8].copy_from_slice(&4_u16.to_le_bytes());
        system[0xD8..0xDC].copy_from_slice(&0x3154_5844_u32.to_le_bytes());
        system[0xDD] = 1;
        system[0xF0..0xF8].copy_from_slice(&0x6000_0000_u64.to_le_bytes());
        system[0x100..0x105].copy_from_slice(b"test\0");

        let parsed = parse_ytd_resource_pages(&system, &graphics, sizes).unwrap();
        assert_eq!(parsed.textures.len(), 1);
        let texture = &parsed.textures[0];
        assert_eq!(texture.name, "test");
        assert_eq!(texture.name_hash, 0x1234_ABCD);
        assert_eq!((texture.width, texture.height, texture.format_code), (4, 4, 0x3154_5844));
        assert_eq!(texture.pixel_data_bytes, 16);
        assert_eq!(texture.pixel_data.unwrap().segment, crate::Rsc7Segment::Graphics);
    }

    #[test]
    fn rejects_dictionary_counts_and_invalid_texture_data_ranges() {
        let sizes = Rpf7ResourcePageSizes { system_bytes: 128, graphics_bytes: 8 };
        let mut system = vec![0_u8; 128];
        system[0x38..0x3A].copy_from_slice(&2_u16.to_le_bytes());
        system[0x3A..0x3C].copy_from_slice(&1_u16.to_le_bytes());
        assert!(parse_ytd_resource_pages(&system, &vec![0; 8], sizes).is_err());

        let sizes = Rpf7ResourcePageSizes { system_bytes: 512, graphics_bytes: 8 };
        let mut system = vec![0_u8; 512];
        system[0x30..0x38].copy_from_slice(&0x5000_0040_u64.to_le_bytes());
        system[0x38..0x3A].copy_from_slice(&1_u16.to_le_bytes());
        system[0x3A..0x3C].copy_from_slice(&1_u16.to_le_bytes());
        system[0x40..0x48].copy_from_slice(&0x5000_0080_u64.to_le_bytes());
        system[0x80 + 0x28..0x80 + 0x30].copy_from_slice(&0_u64.to_le_bytes());
        system[0x80 + 0x50..0x80 + 0x52].copy_from_slice(&4_u16.to_le_bytes());
        system[0x80 + 0x52..0x80 + 0x54].copy_from_slice(&4_u16.to_le_bytes());
        system[0x80 + 0x56..0x80 + 0x58].copy_from_slice(&4_u16.to_le_bytes());
        system[0x80 + 0x5D] = 1;
        system[0x80 + 0x70..0x80 + 0x78].copy_from_slice(&0x6000_0007_u64.to_le_bytes());
        assert!(parse_ytd_resource_pages(&system, &vec![0; 8], sizes).is_err());
    }
}
