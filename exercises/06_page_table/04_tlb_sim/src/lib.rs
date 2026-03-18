//! # TLB Simulation
//!
//! This exercise simulates TLB lookup, insert, eviction, flushing, and MMU
//! interaction with a simplified page table.

#[derive(Clone, Debug)]
pub struct TlbEntry {
    pub valid: bool,
    pub asid: u16,
    pub vpn: u64,
    pub ppn: u64,
    pub flags: u64,
}

impl TlbEntry {
    pub fn empty() -> Self {
        Self {
            valid: false,
            asid: 0,
            vpn: 0,
            ppn: 0,
            flags: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct TlbStats {
    pub hits: u64,
    pub misses: u64,
}

impl TlbStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

pub struct Tlb {
    entries: Vec<TlbEntry>,
    capacity: usize,
    fifo_ptr: usize,
    pub stats: TlbStats,
}

impl Tlb {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: vec![TlbEntry::empty(); capacity],
            capacity,
            fifo_ptr: 0,
            stats: TlbStats::default(),
        }
    }

    pub fn lookup(&mut self, vpn: u64, asid: u16) -> Option<u64> {
        for entry in &self.entries {
            if entry.valid && entry.vpn == vpn && entry.asid == asid {
                self.stats.hits += 1;
                return Some(entry.ppn);
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn insert(&mut self, vpn: u64, ppn: u64, asid: u16, flags: u64) {
        for entry in &mut self.entries {
            if entry.valid && entry.vpn == vpn && entry.asid == asid {
                *entry = TlbEntry {
                    valid: true,
                    asid,
                    vpn,
                    ppn,
                    flags,
                };
                return;
            }
        }

        self.entries[self.fifo_ptr] = TlbEntry {
            valid: true,
            asid,
            vpn,
            ppn,
            flags,
        };
        self.fifo_ptr = (self.fifo_ptr + 1) % self.capacity;
    }

    pub fn flush_all(&mut self) {
        for entry in &mut self.entries {
            entry.valid = false;
        }
    }

    pub fn flush_by_vpn(&mut self, vpn: u64) {
        for entry in &mut self.entries {
            if entry.valid && entry.vpn == vpn {
                entry.valid = false;
            }
        }
    }

    pub fn flush_by_asid(&mut self, asid: u16) {
        for entry in &mut self.entries {
            if entry.valid && entry.asid == asid {
                entry.valid = false;
            }
        }
    }

    pub fn valid_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.valid).count()
    }
}

pub struct PageMapping {
    pub vpn: u64,
    pub ppn: u64,
    pub flags: u64,
}

pub struct Mmu {
    pub tlb: Tlb,
    page_table: Vec<(u16, PageMapping)>,
    pub current_asid: u16,
}

impl Mmu {
    pub fn new(tlb_capacity: usize) -> Self {
        Self {
            tlb: Tlb::new(tlb_capacity),
            page_table: Vec::new(),
            current_asid: 0,
        }
    }

    pub fn add_mapping(&mut self, asid: u16, vpn: u64, ppn: u64, flags: u64) {
        self.page_table
            .push((asid, PageMapping { vpn, ppn, flags }));
    }

    pub fn switch_asid(&mut self, new_asid: u16) {
        self.current_asid = new_asid;
    }

    pub fn translate(&mut self, vpn: u64) -> Option<u64> {
        if let Some(ppn) = self.tlb.lookup(vpn, self.current_asid) {
            return Some(ppn);
        }

        for (asid, mapping) in &self.page_table {
            if *asid == self.current_asid && mapping.vpn == vpn {
                self.tlb
                    .insert(vpn, mapping.ppn, self.current_asid, mapping.flags);
                return Some(mapping.ppn);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_empty_lookup() {
        let mut tlb = Tlb::new(4);
        assert_eq!(tlb.lookup(0x100, 0), None);
        assert_eq!(tlb.stats.misses, 1);
        assert_eq!(tlb.stats.hits, 0);
    }

    #[test]
    fn test_tlb_insert_and_lookup() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x7);
        assert_eq!(tlb.lookup(0x100, 1), Some(0x200));
        assert_eq!(tlb.stats.hits, 1);
    }

    #[test]
    fn test_tlb_asid_isolation() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x7);
        tlb.insert(0x100, 0x300, 2, 0x7);

        assert_eq!(tlb.lookup(0x100, 1), Some(0x200));
        assert_eq!(tlb.lookup(0x100, 2), Some(0x300));
    }

    #[test]
    fn test_tlb_miss_wrong_asid() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x7);

        assert_eq!(tlb.lookup(0x100, 99), None);
        assert_eq!(tlb.stats.misses, 1);
    }

    #[test]
    fn test_tlb_fifo_eviction() {
        let mut tlb = Tlb::new(2);
        tlb.insert(0x10, 0x20, 0, 0x7);
        tlb.insert(0x30, 0x40, 0, 0x7);
        tlb.insert(0x50, 0x60, 0, 0x7);

        assert_eq!(tlb.lookup(0x10, 0), None);
        assert_eq!(tlb.lookup(0x30, 0), Some(0x40));
        assert_eq!(tlb.lookup(0x50, 0), Some(0x60));
    }

    #[test]
    fn test_tlb_update_existing() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x3);
        tlb.insert(0x100, 0x999, 1, 0x7);

        assert_eq!(tlb.lookup(0x100, 1), Some(0x999));
        assert_eq!(tlb.valid_count(), 1);
    }

    #[test]
    fn test_tlb_valid_count() {
        let mut tlb = Tlb::new(4);
        assert_eq!(tlb.valid_count(), 0);

        tlb.insert(0x1, 0x10, 0, 0x7);
        assert_eq!(tlb.valid_count(), 1);

        tlb.insert(0x2, 0x20, 0, 0x7);
        assert_eq!(tlb.valid_count(), 2);
    }

    #[test]
    fn test_flush_all() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x1, 0x10, 0, 0x7);
        tlb.insert(0x2, 0x20, 1, 0x7);
        tlb.insert(0x3, 0x30, 2, 0x7);
        assert_eq!(tlb.valid_count(), 3);

        tlb.flush_all();
        assert_eq!(tlb.valid_count(), 0);
        assert_eq!(tlb.lookup(0x1, 0), None);
    }

    #[test]
    fn test_flush_by_vpn() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x7);
        tlb.insert(0x100, 0x300, 2, 0x7);
        tlb.insert(0x999, 0x400, 1, 0x7);

        tlb.flush_by_vpn(0x100);

        assert_eq!(tlb.lookup(0x100, 1), None);
        assert_eq!(tlb.lookup(0x100, 2), None);
        assert_eq!(tlb.lookup(0x999, 1), Some(0x400));
    }

    #[test]
    fn test_flush_by_asid() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x1, 0x10, 1, 0x7);
        tlb.insert(0x2, 0x20, 1, 0x7);
        tlb.insert(0x3, 0x30, 2, 0x7);

        tlb.flush_by_asid(1);

        assert_eq!(tlb.lookup(0x1, 1), None);
        assert_eq!(tlb.lookup(0x2, 1), None);
        assert_eq!(tlb.lookup(0x3, 2), Some(0x30));
    }

    #[test]
    fn test_flush_by_vpn_then_reinsert() {
        let mut tlb = Tlb::new(4);
        tlb.insert(0x100, 0x200, 1, 0x7);
        tlb.flush_by_vpn(0x100);
        assert_eq!(tlb.lookup(0x100, 1), None);

        tlb.insert(0x100, 0x500, 1, 0x7);
        assert_eq!(tlb.lookup(0x100, 1), Some(0x500));
    }

    #[test]
    fn test_mmu_basic_translate() {
        let mut mmu = Mmu::new(4);
        mmu.current_asid = 1;
        mmu.add_mapping(1, 0x100, 0x200, 0x7);

        let ppn = mmu.translate(0x100);
        assert_eq!(ppn, Some(0x200));
        assert_eq!(mmu.tlb.stats.misses, 1);
        assert_eq!(mmu.tlb.stats.hits, 0);

        let ppn = mmu.translate(0x100);
        assert_eq!(ppn, Some(0x200));
        assert_eq!(mmu.tlb.stats.hits, 1);
    }

    #[test]
    fn test_mmu_page_fault() {
        let mut mmu = Mmu::new(4);
        mmu.current_asid = 1;
        assert_eq!(mmu.translate(0x999), None);
    }

    #[test]
    fn test_mmu_asid_switch() {
        let mut mmu = Mmu::new(4);
        mmu.add_mapping(1, 0x100, 0x200, 0x7);
        mmu.add_mapping(2, 0x100, 0x300, 0x7);

        mmu.switch_asid(1);
        assert_eq!(mmu.translate(0x100), Some(0x200));

        mmu.switch_asid(2);
        assert_eq!(mmu.translate(0x100), Some(0x300));
    }

    #[test]
    fn test_mmu_flush_on_asid_switch() {
        let mut mmu = Mmu::new(4);
        mmu.add_mapping(1, 0x100, 0x200, 0x7);
        mmu.add_mapping(2, 0x100, 0x300, 0x7);

        mmu.switch_asid(1);
        assert_eq!(mmu.translate(0x100), Some(0x200));

        mmu.switch_asid(2);
        mmu.tlb.flush_by_asid(1);

        let old_misses = mmu.tlb.stats.misses;
        assert_eq!(mmu.translate(0x100), Some(0x300));
        assert_eq!(mmu.tlb.stats.misses, old_misses + 1);
    }

    #[test]
    fn test_mmu_hit_rate() {
        let mut mmu = Mmu::new(4);
        mmu.current_asid = 0;
        mmu.add_mapping(0, 0x1, 0x10, 0x7);

        mmu.translate(0x1);
        for _ in 0..9 {
            mmu.translate(0x1);
        }

        assert_eq!(mmu.tlb.stats.hits, 9);
        assert_eq!(mmu.tlb.stats.misses, 1);
        let rate = mmu.tlb.stats.hit_rate();
        assert!(
            (rate - 0.9).abs() < 1e-9,
            "hit rate should be 0.9, got {rate}"
        );
    }

    #[test]
    fn test_mmu_thrashing() {
        let mut mmu = Mmu::new(2);
        mmu.current_asid = 0;
        mmu.add_mapping(0, 0x1, 0x10, 0x7);
        mmu.add_mapping(0, 0x2, 0x20, 0x7);
        mmu.add_mapping(0, 0x3, 0x30, 0x7);

        for vpn in [1, 2, 3, 1, 2, 3] {
            mmu.translate(vpn);
        }

        assert_eq!(mmu.tlb.stats.misses, 6);
        assert_eq!(mmu.tlb.stats.hits, 0);
    }
}
