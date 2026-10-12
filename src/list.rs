//! Linked-list benchmark, mirroring `core_list_join.c`.
//!
//! The reference implementation builds a linked list by carving cells out of a
//! flat memory block. Here the cells live in a single arena and reference each
//! other by index, which removes the unsafe pointer plumbing and the double
//! indirection between nodes and their data, while preserving the exact
//! traversal and mutation order of the C code.
//!
//! List traversal is the hottest code in CoreMark. To keep the working set small
//! the `next` link is a `u32` where [`NIL`] marks "no successor" instead of an
//! `Option<u32>`, so a cell is 8 bytes rather than 12. The traversal loops index
//! the arena through [`List::at`]/[`List::at_mut`], which skip bounds checks
//! using an invariant documented on those methods.

use crate::matrix::core_bench_matrix;
use crate::state::core_bench_state;
use crate::types::{CoreResults, ListData};
use crate::util::{crc16, crcu16};

/// Bytes per linked-list cell used to derive the number of elements, matching
/// the portable `list_head` + `list_data` layout of the reference.
const PER_ITEM: usize = 16 + 4;

/// Sentinel meaning "no successor".
const NIL: u32 = u32::MAX;

/// One linked-list cell: a `next` index plus the inline data item.
#[derive(Clone, Copy, Default)]
pub struct Cell {
    pub next: u32,
    pub data16: i16,
    pub idx: i16,
}

/// A linked list backed by a single cell arena.
pub struct List {
    /// Index of the list head; always a valid cell.
    pub head: u32,
    pub cells: Vec<Cell>,
}

impl List {
    /// Bounds-check-free cell read. Every index stored in the arena is produced
    /// by `insert_new`, which pushes exactly one cell per index, so indices are
    /// always either valid or [`NIL`]; callers resolve [`NIL`] to `None` first.
    #[inline(always)]
    fn at(&self, index: u32) -> &Cell {
        debug_assert!((index as usize) < self.cells.len());
        unsafe { self.cells.get_unchecked(index as usize) }
    }

    /// Bounds-check-free mutable cell read; same invariant as [`List::at`].
    #[inline(always)]
    fn at_mut(&mut self, index: u32) -> &mut Cell {
        debug_assert!((index as usize) < self.cells.len());
        unsafe { self.cells.get_unchecked_mut(index as usize) }
    }

    /// Find an item by index (when `info.idx >= 0`) or by data value.
    fn find(&self, list: u32, info: &ListData) -> Option<u32> {
        let mut cur = list;
        if info.idx >= 0 {
            loop {
                let cell = self.at(cur);
                if cell.idx == info.idx {
                    return Some(cur);
                }
                if cell.next == NIL {
                    return None;
                }
                cur = cell.next;
            }
        } else {
            let want = info.data16 as i32 & 0xff;
            loop {
                let cell = self.at(cur);
                if (cell.data16 as i32 & 0xff) == want {
                    return Some(cur);
                }
                if cell.next == NIL {
                    return None;
                }
                cur = cell.next;
            }
        }
    }

    /// Reverse a list and return the new head.
    fn reverse(&mut self, list: u32) -> u32 {
        let mut next = NIL;
        let mut cur = list;
        loop {
            let cell = self.at_mut(cur);
            let tmp = cell.next;
            cell.next = next;
            next = cur;
            if tmp == NIL {
                return next;
            }
            cur = tmp;
        }
    }

    /// Remove the cell after `item` by swapping the data of the two cells,
    /// mirroring `core_list_remove`.
    ///
    /// A degenerate list can end at `item`, in which case there is nothing to
    /// remove; the reference dereferences a null link there.
    fn remove(&mut self, item: u32) -> u32 {
        let ret = self.at(item).next;
        if ret == NIL {
            return NIL;
        }
        let i = item as usize;
        let r = ret as usize;
        let (di, ii) = (self.cells[i].data16, self.cells[i].idx);
        self.cells[i].data16 = self.cells[r].data16;
        self.cells[i].idx = self.cells[r].idx;
        self.cells[r].data16 = di;
        self.cells[r].idx = ii;
        self.cells[i].next = self.cells[r].next;
        self.cells[r].next = NIL;
        ret
    }

    /// Undo a previous [`List::remove`], mirroring `core_list_undo_remove`.
    fn undo_remove(&mut self, item_removed: u32, item_modified: u32) -> u32 {
        if item_removed == NIL {
            return NIL;
        }
        let r = item_removed as usize;
        let m = item_modified as usize;
        let (dr, ir) = (self.cells[r].data16, self.cells[r].idx);
        self.cells[r].data16 = self.cells[m].data16;
        self.cells[r].idx = self.cells[m].idx;
        self.cells[m].data16 = dr;
        self.cells[m].idx = ir;
        self.cells[r].next = self.cells[m].next;
        self.cells[m].next = item_removed;
        item_removed
    }

    /// In-place iterative mergesort, mirroring `core_list_mergesort`.
    fn mergesort_by<F>(&mut self, mut list: u32, cmp: &mut F) -> u32
    where
        F: FnMut(&mut List, u32, u32) -> i32,
    {
        let mut insize: i64 = 1;
        loop {
            let mut p = list;
            let mut new_list = NIL;
            let mut tail = NIL;
            let mut nmerges: i64 = 0;

            while p != NIL {
                nmerges += 1;

                // Step `insize` places along from p.
                let mut q = p;
                let mut psize: i64 = 0;
                for _ in 0..insize {
                    psize += 1;
                    q = self.at(q).next;
                    if q == NIL {
                        break;
                    }
                }
                let mut qsize: i64 = insize;

                let mut pcur = p;
                while psize > 0 || (qsize > 0 && q != NIL) {
                    // Take from `p` unless its run is exhausted or `q` orders
                    // first. `cmp` has side effects, so it is only consulted
                    // when both runs are still non-empty, as in the reference.
                    let take_p = if psize == 0 {
                        false
                    } else if qsize == 0 || q == NIL {
                        true
                    } else {
                        cmp(self, pcur, q) <= 0
                    };

                    let e = if take_p {
                        let e = pcur;
                        pcur = self.at(pcur).next;
                        psize -= 1;
                        e
                    } else {
                        let e = q;
                        q = self.at(q).next;
                        qsize -= 1;
                        e
                    };

                    // Append `e`. `tail` and the cursors can alias `e`, so this
                    // write happens after the cursor updates above.
                    if tail == NIL {
                        new_list = e;
                    } else {
                        self.at_mut(tail).next = e;
                    }
                    tail = e;
                }

                p = q;
            }

            if tail != NIL {
                self.at_mut(tail).next = NIL;
            }
            list = new_list;
            if nmerges <= 1 {
                return list;
            }
            insize *= 2;
        }
    }

    /// Run the list benchmark once, mirroring `core_bench_list`.
    pub fn bench(&mut self, res: &mut CoreResults, finder_idx: i16) -> u16 {
        let mut retval: u16 = 0;
        let mut found: u16 = 0;
        let mut missed: u16 = 0;
        let mut list = self.head;
        let find_num = res.seed3;
        let mut info = ListData {
            data16: 0,
            idx: finder_idx,
        };

        let mut i: i16 = 0;
        while i < find_num {
            info.data16 = i & 0xff;
            let this_find = self.find(list, &info);
            list = self.reverse(list);
            match this_find {
                None => {
                    missed += 1;
                    let next = self.at(list).next;
                    retval = retval.wrapping_add((self.at(next).data16 >> 8) as u16 & 1);
                }
                Some(tf) => {
                    found += 1;
                    if self.at(tf).data16 & 0x1 != 0 {
                        retval = retval.wrapping_add((self.at(tf).data16 >> 9) as u16 & 1);
                    }
                    // Cache the found item's successor at the head of the list.
                    let finder = self.at(tf).next;
                    if finder != NIL {
                        self.at_mut(tf).next = self.at(finder).next;
                        self.at_mut(finder).next = self.at(list).next;
                        self.at_mut(list).next = finder;
                    }
                }
            }
            if info.idx >= 0 {
                info.idx += 1;
            }
            i += 1;
        }
        retval = retval.wrapping_add(found.wrapping_mul(4).wrapping_sub(missed));

        // Sort by data content and remove one item.
        if finder_idx > 0 {
            list = self.mergesort_by(list, &mut |l, a, b| cmp_complex(l, res, a, b));
        }
        let remover = self.remove(self.at(list).next);

        // CRC the data content from the head, then undo the removal.
        let mut finder = match self.find(list, &info) {
            Some(n) => n,
            None => self.at(list).next,
        };
        while finder != NIL {
            retval = crc16(self.at(list).data16, retval);
            finder = self.at(finder).next;
        }
        self.undo_remove(remover, self.at(list).next);

        // Sort by index, returning the list to its original order.
        list = self.mergesort_by(list, &mut cmp_idx);

        let mut finder = self.at(list).next;
        while finder != NIL {
            retval = crc16(self.at(list).data16, retval);
            finder = self.at(finder).next;
        }

        retval
    }
}

/// Insert a new cell after `insert_point`, respecting the arena limit.
fn insert_new(
    cells: &mut Vec<Cell>,
    insert_point: u32,
    info: ListData,
    limit: usize,
    cell_next: &mut usize,
) -> Option<u32> {
    if *cell_next + 1 >= limit {
        return None;
    }
    let new_index = *cell_next as u32;
    *cell_next += 1;
    let old_next = cells[insert_point as usize].next;
    cells.push(Cell {
        next: old_next,
        data16: info.data16,
        idx: info.idx,
    });
    cells[insert_point as usize].next = new_index;
    Some(new_index)
}

/// Initialize the list with data, mirroring `core_list_init`.
pub fn core_list_init(blksize: usize, seed: i16) -> List {
    let size = (blksize / PER_ITEM).saturating_sub(2);

    let mut cells: Vec<Cell> = Vec::with_capacity(size.max(1));
    cells.push(Cell {
        next: NIL,
        data16: 0x8080u16 as i16,
        idx: 0x0000,
    });
    let mut cell_next = 1usize;

    // Fake tail item. The reference relies on this item always being present;
    // for degenerate block sizes its bounds check would skip it and a later
    // null-link dereference follows, so force it in to stay well defined.
    let tail = ListData {
        idx: 0x7fff,
        data16: 0xffffu16 as i16,
    };
    if insert_new(&mut cells, 0, tail, size, &mut cell_next).is_none() {
        let new_index = cells.len() as u32;
        let old_next = cells[0].next;
        cells.push(Cell {
            next: old_next,
            data16: tail.data16,
            idx: tail.idx,
        });
        cells[0].next = new_index;
    }

    // Real items.
    for i in 0..size {
        let datpat = (((seed as i32) ^ (i as i32)) as u16) & 0xf;
        let dat = (datpat << 3) | (i as u16 & 0x7);
        insert_new(
            &mut cells,
            0,
            ListData {
                data16: ((dat << 8) | dat) as i16,
                idx: 0,
            },
            size,
            &mut cell_next,
        );
    }

    let mut list = List { head: 0, cells };

    // Index the list so the initial seed order is known. The fake head and tail
    // are skipped, matching the reference `finder = list->next` traversal.
    let mut finder = list.at(list.head).next;
    let mut i: u32 = 1;
    while finder != NIL && list.at(finder).next != NIL {
        if (i as usize) < size / 5 {
            list.at_mut(finder).idx = i as i16;
            i += 1;
        } else {
            let pat = (i ^ (seed as u32)) as u16;
            i += 1;
            list.at_mut(finder).idx = (0x3fff & (((i & 0x07) << 8) | pat as u32)) as i16;
        }
        finder = list.at(finder).next;
    }

    list.head = list.mergesort_by(list.head, &mut cmp_idx);
    list
}

/// Compare data items by invoking the appropriate algorithm, mirroring `cmp_complex`.
fn cmp_complex(list: &mut List, res: &mut CoreResults, a: u32, b: u32) -> i32 {
    let mut da = list.at(a).data16;
    let val1 = calc_func(res, &mut da);
    list.at_mut(a).data16 = da;

    let mut db = list.at(b).data16;
    let val2 = calc_func(res, &mut db);
    list.at_mut(b).data16 = db;

    (val1 - val2) as i32
}

/// Compare data items by index and regenerate their data, mirroring `cmp_idx`.
fn cmp_idx(list: &mut List, a: u32, b: u32) -> i32 {
    for node in [a, b] {
        let raw = list.at(node).data16 as i32;
        list.at_mut(node).data16 = ((raw & 0xff00) | (0x00ff & (raw >> 8))) as i16;
    }
    list.at(a).idx as i32 - list.at(b).idx as i32
}

/// Compute a list item's value, caching the result, mirroring `calc_func`.
fn calc_func(res: &mut CoreResults, pdata: &mut i16) -> i16 {
    let data = *pdata;
    let optype = (data >> 7) & 1;
    if optype != 0 {
        return data & 0x007f;
    }

    let flag = data & 0x7;
    let mut dtype = (data >> 3) & 0xf;
    dtype |= dtype << 4;

    let retval: i16 = match flag {
        0 => {
            let dtype = if dtype < 0x22 { 0x22 } else { dtype };
            let size = res.size;
            let r = core_bench_state(
                size,
                &mut res.state_mem,
                res.seed1,
                res.seed2,
                dtype,
                res.crc,
            );
            if res.crcstate == 0 {
                res.crcstate = r;
            }
            r as i16
        }
        1 => {
            let r = core_bench_matrix(&mut res.mat, dtype, res.crc);
            if res.crcmatrix == 0 {
                res.crcmatrix = r;
            }
            r as i16
        }
        _ => data,
    };

    res.crc = crcu16(retval as u16, res.crc);
    let out = retval & 0x007f;
    *pdata = (((data as u16) & 0xff00) | 0x0080 | out as u16) as i16;
    out
}
