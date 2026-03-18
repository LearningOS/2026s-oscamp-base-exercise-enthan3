//! # SV39 Three-Level Page Table
//!
//! This exercise simulates a RISC-V SV39 page table using `HashMap`-backed
//! page table nodes.

use std::collections::HashMap;

pub const PAGE_SIZE: usize = 4096;
pub const PT_ENTRIES: usize = 512;

pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;

const PPN_SHIFT: u32 = 10;

#[derive(Clone)]
pub struct PageTableNode {
    pub entries: [u64; PT_ENTRIES],
}

impl PageTableNode {
    pub fn new() -> Self {
        Self {
            entries: [0; PT_ENTRIES],
        }
    }
}

impl Default for PageTableNode {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Sv39PageTable {
    nodes: HashMap<u64, PageTableNode>,
    pub root_ppn: u64,
    next_ppn: u64,
}

#[derive(Debug, PartialEq)]
pub enum TranslateResult {
    Ok(u64),
    PageFault,
}

impl Sv39PageTable {
    pub fn new() -> Self {
        let mut pt = Self {
            nodes: HashMap::new(),
            root_ppn: 0x80000,
            next_ppn: 0x80001,
        };
        pt.nodes.insert(pt.root_ppn, PageTableNode::new());
        pt
    }

    fn alloc_node(&mut self) -> u64 {
        let ppn = self.next_ppn;
        self.next_ppn += 1;
        self.nodes.insert(ppn, PageTableNode::new());
        ppn
    }

    fn is_leaf(pte: u64) -> bool {
        pte & (PTE_R | PTE_W | PTE_X) != 0
    }

    fn pte_ppn(pte: u64) -> u64 {
        pte >> PPN_SHIFT
    }

    pub fn extract_vpn(va: u64, level: usize) -> usize {
        ((va >> (12 + level * 9)) & 0x1FF) as usize
    }

    pub fn map_page(&mut self, va: u64, pa: u64, flags: u64) {
        let va = va & !((PAGE_SIZE as u64) - 1);
        let pa = pa & !((PAGE_SIZE as u64) - 1);

        let mut current_ppn = self.root_ppn;
        for level in [2usize, 1] {
            let idx = Self::extract_vpn(va, level);
            let pte = self
                .nodes
                .get(&current_ppn)
                .expect("page table node must exist")
                .entries[idx];

            let next_ppn = if pte & PTE_V == 0 {
                let new_ppn = self.alloc_node();
                self.nodes
                    .get_mut(&current_ppn)
                    .expect("page table node must exist")
                    .entries[idx] = (new_ppn << PPN_SHIFT) | PTE_V;
                new_ppn
            } else {
                Self::pte_ppn(pte)
            };

            current_ppn = next_ppn;
        }

        let idx = Self::extract_vpn(va, 0);
        self.nodes
            .get_mut(&current_ppn)
            .expect("leaf parent node must exist")
            .entries[idx] = ((pa >> 12) << PPN_SHIFT) | flags;
    }

    pub fn translate(&self, va: u64) -> TranslateResult {
        let mut current_ppn = self.root_ppn;

        for level in [2usize, 1, 0] {
            let node = match self.nodes.get(&current_ppn) {
                Some(node) => node,
                None => return TranslateResult::PageFault,
            };
            let pte = node.entries[Self::extract_vpn(va, level)];

            if pte & PTE_V == 0 {
                return TranslateResult::PageFault;
            }

            if Self::is_leaf(pte) {
                let offset_bits = 12 + level * 9;
                let offset_mask = (1u64 << offset_bits) - 1;
                let offset = va & offset_mask;
                return TranslateResult::Ok((Self::pte_ppn(pte) << 12) | offset);
            }

            if level == 0 {
                return TranslateResult::PageFault;
            }

            current_ppn = Self::pte_ppn(pte);
        }

        TranslateResult::PageFault
    }

    pub fn map_superpage(&mut self, va: u64, pa: u64, flags: u64) {
        let mega_size: u64 = (PAGE_SIZE * PT_ENTRIES) as u64;
        assert_eq!(va % mega_size, 0, "va must be 2MB-aligned");
        assert_eq!(pa % mega_size, 0, "pa must be 2MB-aligned");

        let mut current_ppn = self.root_ppn;
        let idx = Self::extract_vpn(va, 2);
        let pte = self
            .nodes
            .get(&current_ppn)
            .expect("root node must exist")
            .entries[idx];

        if pte & PTE_V == 0 {
            let new_ppn = self.alloc_node();
            self.nodes
                .get_mut(&current_ppn)
                .expect("root node must exist")
                .entries[idx] = (new_ppn << PPN_SHIFT) | PTE_V;
            current_ppn = new_ppn;
        } else {
            current_ppn = Self::pte_ppn(pte);
        }

        let idx = Self::extract_vpn(va, 1);
        self.nodes
            .get_mut(&current_ppn)
            .expect("level-1 node must exist")
            .entries[idx] = ((pa >> 12) << PPN_SHIFT) | flags;
    }
}

impl Default for Sv39PageTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_vpn() {
        let va: u64 = 0x7FFFFFF000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0x1FF);
    }

    #[test]
    fn test_extract_vpn_simple() {
        let va: u64 = 0x1000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 1);
    }

    #[test]
    fn test_extract_vpn_level2() {
        let va: u64 = 0x40000000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 1);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0);
    }

    #[test]
    fn test_map_and_translate_single() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);

        let result = pt.translate(0x1000);
        assert_eq!(result, TranslateResult::Ok(0x80001000));
    }

    #[test]
    fn test_translate_with_offset() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x2000, 0x90000000, PTE_V | PTE_R | PTE_W);

        let result = pt.translate(0x2ABC);
        assert_eq!(result, TranslateResult::Ok(0x90000ABC));
    }

    #[test]
    fn test_translate_page_fault() {
        let pt = Sv39PageTable::new();
        assert_eq!(pt.translate(0x1000), TranslateResult::PageFault);
    }

    #[test]
    fn test_multiple_mappings() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x0000_1000, 0x8000_1000, PTE_V | PTE_R);
        pt.map_page(0x0000_2000, 0x8000_5000, PTE_V | PTE_R | PTE_W);
        pt.map_page(0x0040_0000, 0x9000_0000, PTE_V | PTE_R);

        assert_eq!(pt.translate(0x1234), TranslateResult::Ok(0x80001234));
        assert_eq!(pt.translate(0x2000), TranslateResult::Ok(0x80005000));
        assert_eq!(pt.translate(0x400100), TranslateResult::Ok(0x90000100));
    }

    #[test]
    fn test_map_overwrite() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x80001000));

        pt.map_page(0x1000, 0x90002000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x90002000));
    }

    #[test]
    fn test_superpage_mapping() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0x200000, 0x80200000, PTE_V | PTE_R | PTE_W);

        assert_eq!(pt.translate(0x200000), TranslateResult::Ok(0x80200000));
        assert_eq!(pt.translate(0x200ABC), TranslateResult::Ok(0x80200ABC));
        assert_eq!(pt.translate(0x2FF000), TranslateResult::Ok(0x802FF000));
    }

    #[test]
    fn test_superpage_and_normal_coexist() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0x0, 0x80000000, PTE_V | PTE_R);
        pt.map_page(0x40000000, 0x90001000, PTE_V | PTE_R);

        assert_eq!(pt.translate(0x100), TranslateResult::Ok(0x80000100));
        assert_eq!(pt.translate(0x40000000), TranslateResult::Ok(0x90001000));
    }
}
