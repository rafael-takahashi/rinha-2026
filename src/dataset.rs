use crate::config::D;
use kiddo::immutable::float::kdtree::ArchivedR8ImmutableKdTree;
use memmap2::Mmap;
use std::fs::{File, read};

pub struct Dataset {
    labels: Vec<u8>,
    kd_tree_mmap: Mmap,
}

impl Dataset {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let labels = read("data/labels.bin")?;
        let kd_tree_file = File::open("data/tree.rkyv")?;
        let kd_tree_mmap = unsafe { Mmap::map(&kd_tree_file)? };
        Ok(Dataset {
            labels,
            kd_tree_mmap,
        })
    }

    pub fn labels(&self) -> &[u8] {
        &self.labels
    }

    pub fn kd_tree(&self) -> &ArchivedR8ImmutableKdTree<f32, u32, D, 32> {
        unsafe { rkyv::access_unchecked(&self.kd_tree_mmap[..]) }
    }
}
