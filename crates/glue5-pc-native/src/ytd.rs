//! Experimental bounded metadata parser for a presumed GTA V Legacy YTD layout.
//!
//! The field offsets in this local parser are not yet verified against retail
//! assets. It parses RSC7 page descriptors and validates virtual-pointer ranges;
//! it does not decode BC/DDS pixel data. Prefer a retail-validated parser before
//! treating these results as authoritative game texture metadata.

use crate::{
    read_rsc7_pointer_from_pages, resolve_rsc7_pointer, ManifestError,
    Rpf7ResourcePageSizes, Rsc7ResolvedPointer,
};

const MAX_TEXTURES: usize = 4096;
const MAX_NAME_BYTES: u32 = 256;
const DICTIONARY_HEADER_BYTES: u32 = 0x40;
const TEXTURE_HEADER_BYTES: u32 = 0x90;

/// Known legacy GTA texture pixel-format identifiers. Unknown values are
/// preserved by the parser rather than rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YtdPixelFormat {
    A8R8G8B8,
    X8R8G8B8,
    A1R5G5B5,
    A8,
    A8B8G8R8,
    L8,
    Dxt1,
    Dxt3,
    Dxt5,
    Ati1,
    Ati2,
    Bc7,
    Unknown(u32),
}

impl YtdPixelFormat {
    pub fn from_code(code: u32) -> Self {
        match code {
            21 => Self::A8R8G8B8,
            22 => Self::X8R8G8B8,
            25 => Self::A1R5G5B5,
            28 => Self::A8,
            32 => Self::A8B8G8R8,
            50 => Self::L8,
            0x3154_5844 => Self::Dxt1,
            0x3354_5844 => Self::Dxt3,
            0x3554_5844 => Self::Dxt5,
            0x3149_5441 => Self::Ati1,
            0x3249_5441 => Self::Ati2,
            0x2037_4342 => Self::Bc7,
            other => Self::Unknown(other),
        }
    }
}

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


/// Decode the top mip of supported uncompressed legacy YTD pixels to RGBA8.
/// Pixel bytes and metadata must come from a separately validated source.
pub fn decode_ytd_uncompressed_rgba(
    width: u16, height: u16, stride: u16, format_code: u32,
    pixel_data: &[u8], max_output_bytes: usize,
) -> Result<Vec<u8>, ManifestError> {
    let (width, height, stride) = (usize::from(width), usize::from(height), usize::from(stride));
    if width == 0 || height == 0 || stride == 0 {
        return Err(ManifestError("YTD dimensions and stride must be non-zero".into()));
    }
    let output_len = width.checked_mul(height).and_then(|n| n.checked_mul(4))
        .ok_or_else(|| ManifestError("YTD RGBA output size overflow".into()))?;
    if output_len > max_output_bytes {
        return Err(ManifestError("YTD RGBA output exceeds configured limit".into()));
    }
    let format = YtdPixelFormat::from_code(format_code);
    let bpp = match format {
        YtdPixelFormat::A8R8G8B8 | YtdPixelFormat::X8R8G8B8 | YtdPixelFormat::A8B8G8R8 => 4,
        YtdPixelFormat::A1R5G5B5 => 2,
        YtdPixelFormat::A8 | YtdPixelFormat::L8 => 1,
        _ => return Err(ManifestError("YTD format is compressed or unsupported by this decoder".into())),
    };
    let row_bytes = width.checked_mul(bpp)
        .ok_or_else(|| ManifestError("YTD row size overflow".into()))?;
    if stride < row_bytes {
        return Err(ManifestError("YTD stride is smaller than its pixel row".into()));
    }
    let input_len = stride.checked_mul(height)
        .ok_or_else(|| ManifestError("YTD input size overflow".into()))?;
    if pixel_data.len() < input_len {
        return Err(ManifestError("YTD top mip pixel data is truncated".into()));
    }
    let mut rgba = Vec::with_capacity(output_len);
    for y in 0..height {
        let row = &pixel_data[y * stride..y * stride + row_bytes];
        match format {
            YtdPixelFormat::A8R8G8B8 => for p in row.chunks_exact(4) { rgba.extend_from_slice(&[p[2], p[1], p[0], p[3]]); },
            YtdPixelFormat::X8R8G8B8 => for p in row.chunks_exact(4) { rgba.extend_from_slice(&[p[2], p[1], p[0], 255]); },
            YtdPixelFormat::A8B8G8R8 => rgba.extend_from_slice(row),
            YtdPixelFormat::L8 => for &v in row { rgba.extend_from_slice(&[v, v, v, 255]); },
            YtdPixelFormat::A8 => for &v in row { rgba.extend_from_slice(&[255, 255, 255, v]); },
            YtdPixelFormat::A1R5G5B5 => for p in row.chunks_exact(2) {
                let value = u16::from_le_bytes([p[0], p[1]]);
                let expand = |v: u16| ((v << 3) | (v >> 2)) as u8;
                rgba.extend_from_slice(&[expand((value >> 10) & 31), expand((value >> 5) & 31), expand(value & 31), if value & 0x8000 != 0 { 255 } else { 0 }]);
            },
            _ => unreachable!("format was validated above"),
        }
    }
    if rgba.len() != output_len {
        return Err(ManifestError("YTD decoder produced inconsistent RGBA length".into()));
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_uncompressed_ytd_top_mip_to_rgba() {
        assert_eq!(decode_ytd_uncompressed_rgba(1, 1, 4, 21, &[0x11, 0x22, 0x33, 0x44], 4).unwrap(), vec![0x33, 0x22, 0x11, 0x44]);
        assert_eq!(decode_ytd_uncompressed_rgba(1, 1, 1, 50, &[0x7A], 4).unwrap(), vec![0x7A, 0x7A, 0x7A, 255]);
        assert_eq!(decode_ytd_uncompressed_rgba(1, 1, 1, 28, &[0x55], 4).unwrap(), vec![255, 255, 255, 0x55]);
    }

    #[test]
    fn rejects_unsupported_truncated_and_oversized_ytd_pixels() {
        assert!(decode_ytd_uncompressed_rgba(4, 4, 8, 21, &[0; 128], 256).is_err());
        assert!(decode_ytd_uncompressed_rgba(4, 4, 16, 21, &[0; 16], 256).is_err());
        assert!(decode_ytd_uncompressed_rgba(4, 4, 8, 0x3154_5844, &[0; 128], 256).is_err());
        assert!(decode_ytd_uncompressed_rgba(4, 4, 16, 21, &[0; 256], 32).is_err());
    }

    #[test]
    fn maps_known_pixel_formats_and_preserves_unknown_codes() {
        assert_eq!(YtdPixelFormat::from_code(0x3154_5844), YtdPixelFormat::Dxt1);
        assert_eq!(YtdPixelFormat::from_code(0x2037_4342), YtdPixelFormat::Bc7);
        assert_eq!(YtdPixelFormat::from_code(0xDEAD_BEEF), YtdPixelFormat::Unknown(0xDEAD_BEEF));
    }

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
