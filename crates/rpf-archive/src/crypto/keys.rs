use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

/// GTA V keys loaded from the three standard CodeWalker key files.
pub struct GtaKeys {
    pub aes_key: [u8; 32],
    pub ng_keys: Vec<Vec<u8>>,
    pub ng_decrypt_tables: Box<[[[u32; 256]; 16]; 17]>,
}

impl GtaKeys {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let aes: [u8; 32] = fs::read(path.join("gtav_aes_key.dat"))
            .context("missing gtav_aes_key.dat")?
            .try_into().map_err(|_| anyhow::anyhow!("gtav_aes_key.dat must be 32 bytes"))?;
        let key_bytes = fs::read(path.join("gtav_ng_key.dat")).context("missing gtav_ng_key.dat")?;
        const KEY_SIZE: usize = 272;
        const KEY_COUNT: usize = 101;
        if key_bytes.len() < KEY_SIZE * KEY_COUNT {
            bail!("gtav_ng_key.dat is too short");
        }
        let ng_keys = (0..KEY_COUNT)
            .map(|i| key_bytes[i * KEY_SIZE..(i + 1) * KEY_SIZE].to_vec())
            .collect();
        let table_bytes = fs::read(path.join("gtav_ng_decrypt_tables.dat"))
            .context("missing gtav_ng_decrypt_tables.dat")?;
        const TABLE_BYTES: usize = 17 * 16 * 256 * 4;
        if table_bytes.len() < TABLE_BYTES {
            bail!("gtav_ng_decrypt_tables.dat is too short");
        }
        let mut tables: Box<[[[u32; 256]; 16]; 17]> =
            vec![[[0u32; 256]; 16]; 17].into_boxed_slice().try_into()
                .map_err(|_| anyhow::anyhow!("cannot allocate NG tables"))?;
        let mut offset = 0;
        for round in tables.iter_mut() {
            for row in round.iter_mut() {
                for value in row.iter_mut() {
                    *value = u32::from_le_bytes(table_bytes[offset..offset + 4].try_into().unwrap());
                    offset += 4;
                }
            }
        }
        Ok(Self { aes_key: aes, ng_keys, ng_decrypt_tables: tables })
    }
}
