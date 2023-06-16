#![deny(warnings)]

use ruc::*;
use serde::{de, Deserialize, Serialize};
use std::{
    collections::{btree_set::Iter as SmallIter, BTreeSet},
    mem,
};
use vsdb::{basic::mapx_ord::MapxOrdIter as LargeIter, KeyEnDeOrdered, MapxOrd};

type Slot = u64;
type SlotFloor = Slot;
type EntryCnt = u64;

/// A `Skip List` like structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(bound = "T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned")]
pub struct SlotDB<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    data: MapxOrd<Slot, DataCtner<T>>,

    // How many entries in this DB
    total: EntryCnt,

    levels: Vec<Level>,

    multiple_step: u64,

    // Switch the inner implementations of the slot direction:
    // - positive => reverse
    // - reverse => positive
    //
    // Positive query usually get better performance,
    // if most scenes are under the reverse mode,
    // then swap the inner logic
    swap_order: bool,
}

impl<T> SlotDB<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    pub fn new(multiple_step: u64, swap_order: bool) -> Self {
        Self {
            data: MapxOrd::new(),
            total: 0,
            levels: vec![],
            multiple_step,
            swap_order,
        }
    }

    pub fn insert(&mut self, mut slot: Slot, t: T) -> Result<()> {
        if self.swap_order {
            slot = swap_order(slot);
        }

        if let Some(top) = self.levels.last() {
            if top.data.len() as u64 > self.multiple_step {
                let newtop = top.data.iter().fold(
                    Level::new(self.levels.len() as u32, self.multiple_step),
                    |mut l, (slot, cnt)| {
                        let slot_floor = slot / l.floor_base * l.floor_base;
                        *l.data.entry(&slot_floor).or_insert(0) += cnt;
                        l
                    },
                );
                self.levels.push(newtop);
            }
        } else {
            let newtop = self.data.iter().fold(
                Level::new(self.levels.len() as u32, self.multiple_step),
                |mut l, (slot, entries)| {
                    let slot_floor = slot / l.floor_base * l.floor_base;
                    *l.data.entry(&slot_floor).or_insert(0) += entries.len() as u64;
                    l
                },
            );
            self.levels.push(newtop);
        };

        if self
            .data
            .entry(&slot)
            .or_insert(DataCtner::default())
            .insert(t)
        {
            self.levels.iter_mut().for_each(|l| {
                let slot_floor = slot / l.floor_base * l.floor_base;
                *l.data.entry(&slot_floor).or_insert(0) += 1;
            });
            self.total += 1;
        }

        Ok(())
    }

    pub fn remove(&mut self, mut slot: Slot, t: &T) {
        if self.swap_order {
            slot = swap_order(slot);
        }

        loop {
            if let Some(top_len) = self.levels.last().map(|top| top.data.len()) {
                if top_len < 2 {
                    self.levels.pop();
                    continue;
                }
            }
            break;
        }

        let (exist, empty) = if let Some(mut d) = self.data.get_mut(&slot) {
            (d.remove(t), d.is_empty())
        } else {
            return;
        };

        if empty {
            self.data.remove(&slot);
        }

        if exist {
            self.levels.iter_mut().for_each(|l| {
                let slot_floor = slot / l.floor_base * l.floor_base;
                let mut cnt = l.data.get_mut(&slot_floor).unwrap();
                if 1 == *cnt {
                    mem::forget(cnt); // for performance
                    l.data.remove(&slot_floor);
                } else {
                    *cnt -= 1;
                }
            });
            self.total -= 1;
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.levels.iter_mut().for_each(|l| {
            l.data.clear();
        });
        self.levels.clear();
        self.total = 0;
    }

    /// Common usages in web services
    pub fn get_entries_by_page(
        &self,
        page_size: u16,
        page_number: u32, // start from 0
        reverse_order: bool,
    ) -> Vec<T> {
        self.get_entries_by_page_slot(None, page_size, page_number, reverse_order)
    }

    /// Common usages in web services
    pub fn get_entries_by_page_slot(
        &self,
        mut slot_itv: Option<[u64; 2]>, // [included, included]
        page_size: u16,
        page_number: u32, // start from 0
        mut reverse_order: bool,
    ) -> Vec<T> {
        if self.swap_order {
            if let Some([a, b]) = slot_itv {
                slot_itv.replace([swap_order(b), swap_order(a)]);
            }
            reverse_order = !reverse_order;
        }

        if 0 == self.total || 0 == page_size {
            return vec![];
        }

        if let Some(itv) = slot_itv {
            self.entry_range_with_slot_itv(itv, page_size, page_number, reverse_order)
        } else {
            self.entry_range(page_size, page_number, reverse_order)
        }
    }

    // Keep it private
    fn entry_range(&self, page_size: u16, page_number: u32, reverse_order: bool) -> Vec<T> {
        let page_number = page_number as u64;
        let page_size = page_size as u64;

        let take_n = page_size as usize;

        // this is safe as the original type of page is u32
        let n_base = page_size * page_number;
        alt!(self.total <= n_base, return vec![]);

        let mut slot_start = if reverse_order { u64::MAX } else { 0 };
        let mut slot_start_inner_idx = n_base as usize;

        for l in self.levels.iter().rev() {
            if reverse_order {
                for (slot, entry_cnt) in l
                    .data
                    .range(..slot_start)
                    .rev()
                    .map(|(s, cnt)| (s, cnt as usize))
                {
                    if entry_cnt > slot_start_inner_idx {
                        break;
                    } else {
                        slot_start = slot;
                        slot_start_inner_idx -= entry_cnt;
                    }
                }
            } else {
                let mut hdr = l.data.range(slot_start..).peekable();
                while let Some(entry_cnt) = hdr.next().map(|(_, cnt)| cnt as usize) {
                    if entry_cnt > slot_start_inner_idx {
                        break;
                    } else {
                        slot_start = hdr.peek().map(|(s, _)| *s).unwrap_or(u64::MAX);
                        slot_start_inner_idx -= entry_cnt;
                    }
                }
            }
        }

        if reverse_order {
            for (slot, entries) in self.data.range(..slot_start).rev() {
                if entries.len() > slot_start_inner_idx {
                    break;
                } else {
                    slot_start = slot;
                    slot_start_inner_idx -= entries.len();
                }
            }
        } else {
            let mut hdr = self.data.range(slot_start..).peekable();
            while let Some(entry_cnt) = hdr.next().map(|(_, entries)| entries.len()) {
                if entry_cnt > slot_start_inner_idx {
                    break;
                } else {
                    slot_start = hdr.peek().map(|(s, _)| *s).unwrap_or(u64::MAX);
                    slot_start_inner_idx -= entry_cnt;
                }
            }
        }

        self.entry_data_range(
            alt!(reverse_order, 0, slot_start),
            alt!(reverse_order, slot_start, u64::MAX),
            slot_start_inner_idx,
            take_n,
            reverse_order,
        )
    }

    // Keep it private
    fn entry_range_with_slot_itv(
        &self,
        slot_itv: [u64; 2], // [included, included]
        page_size: u16,
        page_number: u32,
        reverse_order: bool,
    ) -> Vec<T> {
        let [slot_min, mut slot_max] = slot_itv;
        if slot_max < slot_min {
            return vec![];
        }
        slot_max = slot_max.saturating_add(1);

        let page_number = page_number as u64;
        let page_size = page_size as u64;

        let mut slot_start = if reverse_order { slot_max } else { slot_min };
        let mut slot_start_inner_idx = (page_size * page_number) as usize;

        if reverse_order {
            for (slot, entries) in self.data.range(slot_min..slot_start).rev() {
                if entries.len() > slot_start_inner_idx {
                    break;
                } else {
                    slot_start = slot;
                    slot_start_inner_idx -= entries.len();
                }
            }
        } else {
            let mut hdr = self.data.range(slot_start..slot_max).peekable();
            while let Some(entry_cnt) = hdr.next().map(|(_, entries)| entries.len()) {
                if entry_cnt > slot_start_inner_idx {
                    break;
                } else {
                    slot_start = hdr.peek().map(|(s, _)| *s).unwrap_or(u64::MAX);
                    slot_start_inner_idx -= entry_cnt;
                }
            }
        }

        self.entry_data_range(
            alt!(reverse_order, slot_min, slot_start),
            alt!(reverse_order, slot_start, slot_max),
            slot_start_inner_idx,
            page_size as usize,
            reverse_order,
        )
    }

    // Keep it private
    fn entry_data_range(
        &self,
        slot_start: u64, // included
        slot_end: u64,   // included
        mut slot_start_inner_idx: usize,
        take_n: usize,
        reverse_order: bool,
    ) -> Vec<T> {
        alt!(slot_end < slot_start, return vec![]);
        let mut ret = vec![];

        if reverse_order {
            for (_, entries) in self.data.range(slot_start..slot_end).rev() {
                entries
                    .iter()
                    .rev()
                    .skip(slot_start_inner_idx)
                    .take(take_n - ret.len())
                    .for_each(|entry| ret.push(entry));
                slot_start_inner_idx = 0;
                if ret.len() >= take_n {
                    assert_eq!(ret.len(), take_n);
                    break;
                }
            }
        } else {
            for (_, entries) in self.data.range(slot_start..slot_end) {
                entries
                    .iter()
                    .skip(slot_start_inner_idx)
                    .take(take_n - ret.len())
                    .for_each(|entry| ret.push(entry));
                slot_start_inner_idx = 0;
                if ret.len() >= take_n {
                    assert_eq!(ret.len(), take_n);
                    break;
                }
            }
        }
        ret
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

impl<T> Default for SlotDB<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    fn default() -> Self {
        Self::new(8, false)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(bound = "T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned")]
enum DataCtner<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    Small(BTreeSet<T>),
    Large(MapxOrd<T, ()>),
}

impl<T> DataCtner<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    fn len(&self) -> usize {
        match self {
            Self::Small(i) => i.len(),
            Self::Large(i) => i.len(),
        }
    }

    fn is_empty(&self) -> bool {
        0 == self.len()
    }

    fn insert(&mut self, t: T) -> bool {
        if let Self::Small(i) = self {
            if i.len() > 8 {
                *self = Self::Large(i.iter().fold(MapxOrd::new(), |mut acc, t| {
                    acc.insert(t, &());
                    acc
                }));
            }
        }

        match self {
            Self::Small(i) => i.insert(t),
            Self::Large(i) => i.insert(&t, &()).is_none(),
        }
    }

    fn remove(&mut self, target: &T) -> bool {
        match self {
            Self::Small(i) => i.remove(target),
            Self::Large(i) => i.remove(target).is_some(),
        }
    }

    fn iter(&self) -> DataCtnerIter<T> {
        match self {
            Self::Small(i) => DataCtnerIter::Small(i.iter()),
            Self::Large(i) => DataCtnerIter::Large(i.iter()),
        }
    }
}

impl<T> Default for DataCtner<T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    fn default() -> Self {
        Self::Small(BTreeSet::new())
    }
}

enum DataCtnerIter<'a, T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    Small(SmallIter<'a, T>),
    Large(LargeIter<'a, T, ()>),
}

impl<'a, T> Iterator for DataCtnerIter<'a, T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Small(i) => i.next().cloned(),
            Self::Large(i) => i.next().map(|j| j.0),
        }
    }
}

impl<'a, T> DoubleEndedIterator for DataCtnerIter<'a, T>
where
    T: Clone + Ord + KeyEnDeOrdered + Serialize + de::DeserializeOwned,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Small(i) => i.next_back().cloned(),
            Self::Large(i) => i.next_back().map(|j| j.0),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Level {
    floor_base: u64,
    data: MapxOrd<SlotFloor, EntryCnt>,
}

impl Level {
    fn new(level_idx: u32, multiple_step: u64) -> Self {
        let pow = 1 + level_idx;
        Self {
            floor_base: multiple_step.pow(pow),
            data: MapxOrd::new(),
        }
    }
}

#[inline(always)]
fn swap_order(original_slot_value: Slot) -> Slot {
    Slot::MAX - original_slot_value
}

#[cfg(test)]
mod test;