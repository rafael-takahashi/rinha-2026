use crate::config::D;
use crate::ivf::ArchivedIvfIndex;
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
        kd_tree_mmap.advise(memmap2::Advice::WillNeed)?;
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

/// The production IVF index: a single mmap'd, rkyv-archived file accessed
/// zero-copy. This is what the runtime serves from (the KD-tree `Dataset` above
/// is only used by the offline oracle harness).
pub struct IvfData {
    mmap: Mmap,
}

impl IvfData {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open("data/ivf.rkyv")?;
        let mmap = unsafe { Mmap::map(&file)? };
        mmap.advise(memmap2::Advice::WillNeed)?;
        Ok(IvfData { mmap })
    }

    pub fn index(&self) -> &ArchivedIvfIndex {
        unsafe { rkyv::access_unchecked(&self.mmap[..]) }
    }

    /// Force the entire index resident before serving. `WillNeed` only *hints*
    /// the kernel to prefetch; this touches one byte per 4 KiB page so every
    /// page is actually faulted into this container's RSS, eliminating
    /// cold-start page-fault latency on the first real requests. The index
    /// (~87 MB) fits well under the 165 MB cap, so it stays resident.
    pub fn warm(&self) {
        const PAGE: usize = 4096;
        let touched = self.mmap[..].iter().step_by(PAGE).fold(0u8, |a, &b| a ^ b);
        std::hint::black_box(touched);
    }
}
