//!
//! # VS functions
//!
//! # Examples
//!
//! Used as version-ful: [**moduler level documents**](super)
//!
//! Used as version-less(not recommand, use `MapxRaw` instead):
//!
//! ```
//! use vsdb_core::{VersionName, versioned::mapx_raw::MapxRawVs, VsMgmt};
//!
//! let dir = format!("/tmp/vsdb_testing/{}", rand::random::<u128>());
//! vsdb_core::vsdb_set_base_dir(&dir);
//!
//! let mut l = MapxRawVs::new();
//! l.version_create(VersionName(b"test")).unwrap();
//!
//! l.insert(&[1], &[0]);
//! l.insert(&[1], &[0]);
//! l.insert(&[2], &[0]);
//!
//! l.iter().for_each(|(_, v)| {
//!     assert_eq!(&v[..], &[0]);
//! });
//!
//! l.remove(&[2]);
//! assert_eq!(l.len(), 1);
//!
//! l.clear();
//! assert_eq!(l.len(), 0);
//! ```
//!

mod backend;

#[cfg(test)]
mod test;

use crate::{
    common::{BranchName, ParentBranchName, RawKey, RawValue, VersionName, NULL_ID},
    BranchNameOwned, VersionNameOwned, VsMgmt,
};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    mem::transmute,
    ops::{Deref, DerefMut, RangeBounds},
};

pub use backend::MapxRawVsIter;

/// Advanced `MapxRaw`, with versioned feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapxRawVs {
    inner: backend::MapxRawVs,
}

impl Default for MapxRawVs {
    fn default() -> Self {
        Self::new()
    }
}

impl MapxRawVs {
    /// # Safety
    ///
    /// This API breaks the semantic safety guarantees,
    /// but it is safe to use in a race-free environment.
    #[inline(always)]
    pub unsafe fn shadow(&self) -> Self {
        Self {
            inner: self.inner.shadow(),
        }
    }

    #[inline(always)]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            inner: backend::MapxRawVs::new(),
        }
    }

    /// Insert a KV to the head version of the default branch.
    #[inline(always)]
    pub fn insert(
        &mut self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Option<RawValue>> {
        self.inner.insert(key.as_ref(), value.as_ref()).c(d!())
    }

    /// Insert a KV to the head version of a specified branch.
    #[inline(always)]
    pub fn insert_by_branch(
        &mut self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
        br_name: BranchName,
    ) -> Result<Option<RawValue>> {
        let br_id = self.inner.branch_get_id_by_name(br_name).c(d!())?;
        self.inner
            .insert_by_branch(key.as_ref(), value.as_ref(), br_id)
            .c(d!())
    }

    /// Remove a KV from the head version of the default branch.
    #[inline(always)]
    pub fn remove(&mut self, key: impl AsRef<[u8]>) -> Result<Option<RawValue>> {
        self.inner.remove(key.as_ref()).c(d!())
    }

    /// Remove a KV from the head version of a specified branch.
    #[inline(always)]
    pub fn remove_by_branch(
        &mut self,
        key: impl AsRef<[u8]>,
        br_name: BranchName,
    ) -> Result<Option<RawValue>> {
        let br_id = self.inner.branch_get_id_by_name(br_name).c(d!())?;
        self.inner.remove_by_branch(key.as_ref(), br_id).c(d!())
    }

    /// Get the value of a key from the default branch.
    #[inline(always)]
    pub fn get(&self, key: impl AsRef<[u8]>) -> Option<RawValue> {
        self.inner.get(key.as_ref())
    }

    #[inline(always)]
    pub fn get_mut<'a, T: 'a + AsRef<[u8]> + ?Sized>(
        &'a mut self,
        key: &'a T,
    ) -> Option<ValueMut<'a>> {
        self.get(key.as_ref())
            .map(|v| ValueMut::new(self, key.as_ref(), v))
    }

    #[inline(always)]
    fn gen_mut<'a, T: 'a + AsRef<[u8]> + ?Sized>(
        &'a mut self,
        key: &'a T,
        v: RawValue,
    ) -> ValueMut<'a> {
        ValueMut::new(self, key.as_ref(), v)
    }

    #[inline(always)]
    pub fn entry<'a>(&'a mut self, key: &'a [u8]) -> Entry<'a> {
        Entry { key, hdr: self }
    }

    /// Get the value of a key from the head of a specified branch.
    #[inline(always)]
    pub fn get_by_branch(
        &self,
        key: impl AsRef<[u8]>,
        br_name: BranchName,
    ) -> Option<RawValue> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        self.inner.get_by_branch(key.as_ref(), br_id)
    }

    /// Get the value of a key from a specified version of a specified branch.
    #[inline(always)]
    pub fn get_by_branch_version(
        &self,
        key: impl AsRef<[u8]>,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> Option<RawValue> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        let ver_id = self.inner.version_get_id_by_name(ver_name)?;
        self.inner
            .get_by_branch_version(key.as_ref(), br_id, ver_id)
    }

    /// Get the value of a key from the default branch,
    /// if the target key does not exist, will try to
    /// search a closest value bigger than the target key.
    #[inline(always)]
    pub fn get_ge<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
    ) -> Option<(RawKey, RawValue)> {
        self.inner.get_ge(key.as_ref())
    }

    /// Get the value of a key from the head of a specified branch,
    /// if the target key does not exist, will try to
    /// search a closest value bigger than the target key.
    #[inline(always)]
    pub fn get_ge_by_branch<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
        br_name: BranchName,
    ) -> Option<(RawKey, RawValue)> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        self.inner.get_ge_by_branch(key.as_ref(), br_id)
    }

    /// Get the value of a key from a specified version of a specified branch,
    /// if the target key does not exist, will try to
    /// search a closest value bigger than the target key.
    #[inline(always)]
    pub fn get_ge_by_branch_version<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> Option<(RawKey, RawValue)> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        let ver_id = self.inner.version_get_id_by_name(ver_name)?;
        self.inner
            .get_ge_by_branch_version(key.as_ref(), br_id, ver_id)
    }

    /// Get the value of a key from the default branch,
    /// if the target key does not exist, will try to
    /// search a closest value less than the target key.
    #[inline(always)]
    pub fn get_le<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
    ) -> Option<(RawKey, RawValue)> {
        self.inner.get_le(key.as_ref())
    }

    /// Get the value of a key from the head of a specified branch,
    /// if the target key does not exist, will try to
    /// search a closest value bigger less the target key.
    #[inline(always)]
    pub fn get_le_by_branch<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
        br_name: BranchName,
    ) -> Option<(RawKey, RawValue)> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        self.inner.get_le_by_branch(key.as_ref(), br_id)
    }

    /// Get the value of a key from a specified version of a specified branch,
    /// if the target key does not exist, will try to
    /// search a closest value bigger than the target key.
    #[inline(always)]
    pub fn get_le_by_branch_version<T: AsRef<[u8]> + ?Sized>(
        &self,
        key: &T,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> Option<(RawKey, RawValue)> {
        let br_id = self.inner.branch_get_id_by_name(br_name)?;
        let ver_id = self.inner.version_get_id_by_name(ver_name)?;
        self.inner
            .get_le_by_branch_version(key.as_ref(), br_id, ver_id)
    }

    /// Create an iterator over the default branch.
    #[inline(always)]
    pub fn iter(&self) -> MapxRawVsIter {
        self.inner.iter()
    }

    /// Create a mutable iterator over the default branch.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> MapxRawVsIterMut {
        MapxRawVsIterMut {
            hdr: self as *mut Self,
            iter: self.inner.iter(),
        }
    }

    /// Create an iterator over a specified branch.
    #[inline(always)]
    pub fn iter_by_branch(&self, br_name: BranchName) -> MapxRawVsIter {
        let br_id = self.inner.branch_get_id_by_name(br_name).unwrap_or(NULL_ID);
        self.inner.iter_by_branch(br_id)
    }

    /// Create an iterator over a specified version of a specified branch.
    #[inline(always)]
    pub fn iter_by_branch_version(
        &self,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> MapxRawVsIter {
        let br_id = self.inner.branch_get_id_by_name(br_name).unwrap_or(NULL_ID);
        let ver_id = self
            .inner
            .version_get_id_by_name(ver_name)
            .unwrap_or(NULL_ID);
        self.inner.iter_by_branch_version(br_id, ver_id)
    }

    /// Create a range iterator over the default branch.
    #[inline(always)]
    pub fn range_mut<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a mut self,
        bounds: R,
    ) -> MapxRawVsIterMut<'a> {
        MapxRawVsIterMut {
            hdr: self as *mut Self,
            iter: self.inner.range(bounds),
        }
    }

    /// Create a mutable range iterator over the default branch.
    #[inline(always)]
    pub fn range<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        self.inner.range(bounds)
    }

    /// Create a range iterator over a specified branch.
    #[inline(always)]
    pub fn range_by_branch<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        br_name: BranchName,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        let br_id = self.inner.branch_get_id_by_name(br_name).unwrap_or(NULL_ID);
        self.inner.range_by_branch(br_id, bounds)
    }

    /// Create a range iterator over a specified version of a specified branch.
    #[inline(always)]
    pub fn range_by_branch_version<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        br_name: BranchName,
        ver_name: VersionName,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        let br_id = self.inner.branch_get_id_by_name(br_name).unwrap_or(NULL_ID);
        let ver_id = self
            .inner
            .version_get_id_by_name(ver_name)
            .unwrap_or(NULL_ID);
        self.inner.range_by_branch_version(br_id, ver_id, bounds)
    }

    /// Check if a key exist on the default branch.
    #[inline(always)]
    pub fn contains_key(&self, key: impl AsRef<[u8]>) -> bool {
        self.get(key.as_ref()).is_some()
    }

    /// Check if a key exist on a specified branch.
    #[inline(always)]
    pub fn contains_key_by_branch(
        &self,
        key: impl AsRef<[u8]>,
        br_name: BranchName,
    ) -> bool {
        self.get_by_branch(key.as_ref(), br_name).is_some()
    }

    /// Check if a key exist on a specified version of a specified branch.
    #[inline(always)]
    pub fn contains_key_by_branch_version(
        &self,
        key: impl AsRef<[u8]>,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> bool {
        self.get_by_branch_version(key.as_ref(), br_name, ver_name)
            .is_some()
    }

    /// NOTE: just a stupid O(n) counter, very slow!
    ///
    /// Get the total number of items of the default branch.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// NOTE: just a stupid O(n) counter, very slow!
    ///
    /// Get the total number of items of the head of a specified branch.
    #[inline(always)]
    pub fn len_by_branch(&self, br_name: BranchName) -> usize {
        self.inner
            .branch_get_id_by_name(br_name)
            .map(|id| self.inner.len_by_branch(id))
            .unwrap_or(0)
    }

    /// NOTE: just a stupid O(n) counter, very slow!
    ///
    /// Get the total number of items of a specified version of a specified branch.
    #[inline(always)]
    pub fn len_by_branch_version(
        &self,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> usize {
        self.inner
            .branch_get_id_by_name(br_name)
            .and_then(|br_id| {
                self.inner
                    .version_get_id_by_name(ver_name)
                    .map(|ver_id| self.inner.len_by_branch_version(br_id, ver_id))
            })
            .unwrap_or(0)
    }

    #[inline(always)]
    #[allow(missing_docs)]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    #[inline(always)]
    #[allow(missing_docs)]
    pub fn is_empty_by_branch(&self, br_name: BranchName) -> bool {
        self.iter_by_branch(br_name).next().is_none()
    }

    #[inline(always)]
    #[allow(missing_docs)]
    pub fn is_empty_by_branch_version(
        &self,
        br_name: BranchName,
        ver_name: VersionName,
    ) -> bool {
        self.iter_by_branch_version(br_name, ver_name)
            .next()
            .is_none()
    }

    /// Clear all data, mainly for testing purpose.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl VsMgmt for MapxRawVs {
    /// Create a new version on the default branch.
    #[inline(always)]
    fn version_create(&mut self, ver_name: VersionName) -> Result<()> {
        self.inner.version_create(ver_name.0).c(d!())
    }

    /// Create a new version on a specified branch,
    /// NOTE: the branch must has been created.
    #[inline(always)]
    fn version_create_by_branch(
        &mut self,
        ver_name: VersionName,
        br_name: BranchName,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|br_id| {
                self.inner
                    .version_create_by_branch(ver_name.0, br_id)
                    .c(d!())
            })
    }

    #[inline(always)]
    fn version_exists_globally(&self, ver_name: VersionName) -> bool {
        self.inner
            .version_get_id_by_name(ver_name)
            .map(|verid| self.inner.version_exists_globally(verid))
            .unwrap_or(false)
    }

    /// Check if a verison exists on default branch.
    #[inline(always)]
    fn version_exists(&self, ver_name: VersionName) -> bool {
        self.inner
            .version_get_id_by_name(ver_name)
            .map(|id| self.inner.version_exists(id))
            .unwrap_or(false)
    }

    /// Check if a version exists on a specified branch(include its parents).
    #[inline(always)]
    fn version_exists_on_branch(
        &self,
        ver_name: VersionName,
        br_name: BranchName,
    ) -> bool {
        self.inner
            .branch_get_id_by_name(br_name)
            .and_then(|br_id| {
                self.inner
                    .version_get_id_by_name(ver_name)
                    .map(|ver_id| self.inner.version_exists_on_branch(ver_id, br_id))
            })
            .unwrap_or(false)
    }

    /// Remove the newest version on the default branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn version_pop(&mut self) -> Result<()> {
        self.inner.version_pop().c(d!())
    }

    /// Remove the newest version on a specified branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn version_pop_by_branch(&mut self, br_name: BranchName) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|br_id| self.inner.version_pop_by_branch(br_id).c(d!()))
    }

    /// Merge all changes made by new versions after the base version into the base version.
    ///
    /// # Safety
    ///
    /// It's the caller's duty to ensure that
    /// the `base_version` was created directly by the `br_id`,
    /// and versions newer than the `base_version` are not used by any other branches,
    /// or the data records of other branches may be corrupted.
    #[inline(always)]
    unsafe fn version_rebase(&mut self, base_version: VersionName) -> Result<()> {
        self.inner
            .version_get_id_by_name(base_version)
            .c(d!())
            .and_then(|bv| self.inner.version_rebase(bv).c(d!()))
    }

    /// Merge all changes made by new versions after the base version into the base version.
    ///
    /// # Safety
    ///
    /// It's the caller's duty to ensure that
    /// the `base_version` was created directly by the `br_id`,
    /// and versions newer than the `base_version` are not used by any other branches,
    /// or the data records of other branches may be corrupted.
    #[inline(always)]
    unsafe fn version_rebase_by_branch(
        &mut self,
        base_version: VersionName,
        br_name: BranchName,
    ) -> Result<()> {
        let bv = self.inner.version_get_id_by_name(base_version).c(d!())?;
        let brid = self.inner.branch_get_id_by_name(br_name).c(d!())?;
        self.inner.version_rebase_by_branch(bv, brid).c(d!())
    }

    #[inline(always)]
    fn version_list(&self) -> Result<Vec<VersionNameOwned>> {
        self.inner.version_list().c(d!())
    }

    #[inline(always)]
    fn version_list_by_branch(
        &self,
        br_name: BranchName,
    ) -> Result<Vec<VersionNameOwned>> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|brid| self.inner.version_list_by_branch(brid).c(d!()))
    }

    #[inline(always)]
    fn version_list_globally(&self) -> Vec<VersionNameOwned> {
        self.inner.version_list_globally()
    }

    #[inline(always)]
    fn version_has_change_set(&self, ver_name: VersionName) -> Result<bool> {
        self.inner
            .version_get_id_by_name(ver_name)
            .c(d!("version not found"))
            .and_then(|verid| self.inner.version_has_change_set(verid).c(d!()))
    }

    #[inline(always)]
    fn version_clean_up_globally(&mut self) -> Result<()> {
        self.inner.version_clean_up_globally().c(d!())
    }

    #[inline(always)]
    unsafe fn version_revert_globally(&mut self, ver_name: VersionName) -> Result<()> {
        self.inner
            .version_get_id_by_name(ver_name)
            .c(d!("version not found"))
            .and_then(|verid| self.inner.version_revert_globally(verid).c(d!()))
    }

    #[inline(always)]
    fn version_chgset_trie_root(
        &self,
        br_name: Option<BranchName>,
        ver_name: Option<VersionName>,
    ) -> Result<Vec<u8>> {
        let brid = if let Some(bn) = br_name {
            Some(
                self.inner
                    .branch_get_id_by_name(bn)
                    .c(d!("version not found"))?,
            )
        } else {
            None
        };

        let verid = if let Some(vn) = ver_name {
            Some(
                self.inner
                    .version_get_id_by_name(vn)
                    .c(d!("version not found"))?,
            )
        } else {
            None
        };

        self.inner.version_chgset_trie_root(brid, verid).c(d!())
    }

    /// Create a new branch based on the head of the default branch.
    #[inline(always)]
    fn branch_create(
        &mut self,
        br_name: BranchName,
        ver_name: VersionName,
        force: bool,
    ) -> Result<()> {
        self.inner
            .branch_create(br_name.0, ver_name.0, force)
            .c(d!())
    }

    /// Create a new branch based on the head of a specified branch.
    #[inline(always)]
    fn branch_create_by_base_branch(
        &mut self,
        br_name: BranchName,
        ver_name: VersionName,
        base_br_name: ParentBranchName,
        force: bool,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(BranchName(base_br_name.0))
            .c(d!("base branch not found"))
            .and_then(|base_br_id| {
                self.inner
                    .branch_create_by_base_branch(
                        br_name.0, ver_name.0, base_br_id, force,
                    )
                    .c(d!())
            })
    }

    /// Create a new branch based on a specified version of a specified branch.
    #[inline(always)]
    fn branch_create_by_base_branch_version(
        &mut self,
        br_name: BranchName,
        ver_name: VersionName,
        base_br_name: ParentBranchName,
        base_ver_name: VersionName,
        force: bool,
    ) -> Result<()> {
        let base_br_id = self
            .inner
            .branch_get_id_by_name(BranchName(base_br_name.0))
            .c(d!("base branch not found"))?;
        let base_ver_id = self
            .inner
            .version_get_id_by_name(base_ver_name)
            .c(d!("base vesion not found"))?;
        self.inner
            .branch_create_by_base_branch_version(
                br_name.0,
                ver_name.0,
                base_br_id,
                base_ver_id,
                force,
            )
            .c(d!())
    }

    /// # Safety
    ///
    /// You should create a new version manually before writing to the new branch,
    /// or the data records referenced by other branches may be corrupted.
    #[inline(always)]
    unsafe fn branch_create_without_new_version(
        &mut self,
        br_name: BranchName,
        force: bool,
    ) -> Result<()> {
        self.inner
            .branch_create_without_new_version(br_name.0, force)
            .c(d!())
    }

    /// # Safety
    ///
    /// You should create a new version manually before writing to the new branch,
    /// or the data records referenced by other branches may be corrupted.
    #[inline(always)]
    unsafe fn branch_create_by_base_branch_without_new_version(
        &mut self,
        br_name: BranchName,
        base_br_name: ParentBranchName,
        force: bool,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(BranchName(base_br_name.0))
            .c(d!("base branch not found"))
            .and_then(|base_br_id| {
                self.inner
                    .branch_create_by_base_branch_without_new_version(
                        br_name.0, base_br_id, force,
                    )
                    .c(d!())
            })
    }

    /// # Safety
    ///
    /// You should create a new version manually before writing to the new branch,
    /// or the data records referenced by other branches may be corrupted.
    #[inline(always)]
    unsafe fn branch_create_by_base_branch_version_without_new_version(
        &mut self,
        br_name: BranchName,
        base_br_name: ParentBranchName,
        base_ver_name: VersionName,
        force: bool,
    ) -> Result<()> {
        let base_br_id = self
            .inner
            .branch_get_id_by_name(BranchName(base_br_name.0))
            .c(d!("base branch not found"))?;
        let base_ver_id = self
            .inner
            .version_get_id_by_name(base_ver_name)
            .c(d!("base vesion not found"))?;
        self.inner
            .branch_create_by_base_branch_version_without_new_version(
                br_name.0,
                base_br_id,
                base_ver_id,
                force,
            )
            .c(d!())
    }

    /// Check if a branch exists or not.
    #[inline(always)]
    fn branch_exists(&self, br_name: BranchName) -> bool {
        self.inner
            .branch_get_id_by_name(br_name)
            .map(|id| {
                assert!(self.inner.branch_exists(id));
                true
            })
            .unwrap_or(false)
    }

    /// Check if a branch exists and has versions on it.
    #[inline(always)]
    fn branch_has_versions(&self, br_name: BranchName) -> bool {
        self.inner
            .branch_get_id_by_name(br_name)
            .map(|id| self.inner.branch_has_versions(id))
            .unwrap_or(false)
    }

    /// Remove a branch, remove all changes directly made by this branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn branch_remove(&mut self, br_name: BranchName) -> Result<()> {
        if let Some(br_id) = self.inner.branch_get_id_by_name(br_name) {
            self.inner.branch_remove(br_id).c(d!())
        } else {
            Err(eg!("branch not found"))
        }
    }

    /// Clean up all other branches not in the list.
    #[inline(always)]
    fn branch_keep_only(&mut self, br_names: &[BranchName]) -> Result<()> {
        let br_ids = br_names
            .iter()
            .copied()
            .map(|brname| {
                self.inner
                    .branch_get_id_by_name(brname)
                    .c(d!("version not found"))
            })
            .collect::<Result<BTreeSet<_>>>()?
            .into_iter()
            .collect::<Vec<_>>();
        self.inner.branch_keep_only(&br_ids).c(d!())
    }

    /// Remove all changes directly made by versions(bigger than `last_ver_id`) of this branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn branch_truncate(&mut self, br_name: BranchName) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|br_id| self.inner.branch_truncate(br_id).c(d!()))
    }

    /// Remove all changes directly made by versions(bigger than `last_ver_id`) of this branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn branch_truncate_to(
        &mut self,
        br_name: BranchName,
        last_ver_name: VersionName,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|br_id| {
                self.inner
                    .version_get_id_by_name(last_ver_name)
                    .c(d!("version not found"))
                    .and_then(|last_ver_id| {
                        self.inner.branch_truncate_to(br_id, last_ver_id).c(d!())
                    })
            })
    }

    /// Remove the newest version on a specified branch.
    ///
    /// 'Write'-like operations on branches and versions are different from operations on data.
    ///
    /// 'Write'-like operations on data require recursive tracing of all parent nodes,
    /// while operations on branches and versions are limited to their own perspective,
    /// and should not do any tracing.
    #[inline(always)]
    fn branch_pop_version(&mut self, br_name: BranchName) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|id| self.inner.branch_pop_version(id).c(d!()))
    }

    /// Merge a branch into another.
    #[inline(always)]
    fn branch_merge_to(
        &mut self,
        br_name: BranchName,
        target_br_name: BranchName,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|brid| {
                let target_brid = self
                    .inner
                    .branch_get_id_by_name(target_br_name)
                    .c(d!("target branch not found"))?;
                self.inner.branch_merge_to(brid, target_brid).c(d!())
            })
    }

    /// Merge a branch into another,
    /// even if new different versions have been created on the target branch.
    ///
    /// # Safety
    ///
    /// If new different versions have been created on the target branch,
    /// the data records referenced by other branches may be corrupted.
    #[inline(always)]
    unsafe fn branch_merge_to_force(
        &mut self,
        br_name: BranchName,
        target_br_name: BranchName,
    ) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|brid| {
                let target_brid = self
                    .inner
                    .branch_get_id_by_name(target_br_name)
                    .c(d!("target branch not found"))?;
                self.inner.branch_merge_to_force(brid, target_brid).c(d!())
            })
    }

    /// Make a branch to be default,
    /// all default operations will be applied to it.
    #[inline(always)]
    fn branch_set_default(&mut self, br_name: BranchName) -> Result<()> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|brid| self.inner.branch_set_default(brid).c(d!()))
    }

    #[inline(always)]
    fn branch_is_empty(&self, br_name: BranchName) -> Result<bool> {
        self.inner
            .branch_get_id_by_name(br_name)
            .c(d!("branch not found"))
            .and_then(|brid| self.inner.branch_is_empty(brid).c(d!()))
    }

    #[inline(always)]
    fn branch_list(&self) -> Vec<BranchNameOwned> {
        self.inner.branch_list()
    }

    #[inline(always)]
    fn branch_get_default(&self) -> BranchNameOwned {
        self.inner.branch_get_default_name()
    }

    #[inline(always)]
    unsafe fn branch_swap(
        &mut self,
        branch_1: BranchName,
        branch_2: BranchName,
    ) -> Result<()> {
        self.inner.branch_swap(branch_1.0, branch_2.0).c(d!())
    }

    /// Clean outdated versions out of the default reserved number.
    #[inline(always)]
    fn prune(&mut self, reserved_ver_num: Option<usize>) -> Result<()> {
        self.inner.prune(reserved_ver_num).c(d!())
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct ValueMut<'a> {
    hdr: &'a mut MapxRawVs,
    key: &'a [u8],
    value: RawValue,
}

impl<'a> ValueMut<'a> {
    fn new(hdr: &'a mut MapxRawVs, key: &'a [u8], value: RawValue) -> Self {
        ValueMut { hdr, key, value }
    }
}

impl<'a> Drop for ValueMut<'a> {
    fn drop(&mut self) {
        pnk!(self.hdr.insert(self.key, &self.value));
    }
}

impl<'a> Deref for ValueMut<'a> {
    type Target = RawValue;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a> DerefMut for ValueMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

pub struct Entry<'a> {
    hdr: &'a mut MapxRawVs,
    key: &'a [u8],
}

impl<'a> Entry<'a> {
    pub fn or_insert(self, default: &'a [u8]) -> ValueMut<'a> {
        let hdr = self.hdr as *mut MapxRawVs;
        if let Some(v) = unsafe { &mut *hdr }.get_mut(self.key) {
            v
        } else {
            unsafe { &mut *hdr }.gen_mut(self.key, default.to_vec())
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct ValueIterMut<'a> {
    key: RawKey,
    value: RawValue,
    iter_mut: &'a mut MapxRawVsIterMut<'a>,
}

impl<'a> Drop for ValueIterMut<'a> {
    fn drop(&mut self) {
        let hdr = unsafe { &mut (*self.iter_mut.hdr) };
        pnk!(hdr.insert(&self.key, &self.value));
    }
}

impl<'a> Deref for ValueIterMut<'a> {
    type Target = RawValue;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a> DerefMut for ValueIterMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MapxRawVsIterMut<'a> {
    hdr: *mut MapxRawVs,
    iter: MapxRawVsIter<'a>,
}

impl<'a> Iterator for MapxRawVsIterMut<'a> {
    type Item = (RawKey, ValueIterMut<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let (k, v) = self.iter.next()?;
        let v = ValueIterMut {
            key: k.clone(),
            value: v,
            iter_mut: unsafe { transmute::<&'_ mut Self, &'a mut Self>(self) },
        };
        Some((k, v))
    }
}

impl<'a> DoubleEndedIterator for MapxRawVsIterMut<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (k, v) = self.iter.next_back()?;
        let v = ValueIterMut {
            key: k.clone(),
            value: v,
            iter_mut: unsafe { transmute::<&'_ mut Self, &'a mut Self>(self) },
        };
        Some((k, v))
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////
