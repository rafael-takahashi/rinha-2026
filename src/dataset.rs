use bytemuck::cast_slice;
use memmap2::Mmap;
use std::fs::File;

pub struct Dataset {
    _vectors_mmap: Mmap,
    _labels_mmap: Mmap,
}

impl Dataset {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let vectors = File::open("data/vectors.bin")?;
        let labels = File::open("data/labels.bin")?;

        let vectors_mmap = unsafe { Mmap::map(&vectors)? };
        let labels_mmap = unsafe { Mmap::map(&labels)? };

        Ok(Dataset {
            _vectors_mmap: vectors_mmap,
            _labels_mmap: labels_mmap,
        })
    }

    pub fn vectors(&self) -> &[[f32; 14]] {
        cast_slice(&self._vectors_mmap)
    }

    pub fn labels(&self) -> &[u8] {
        &self._labels_mmap
    }
}
