//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//
use crate::reader::{BacktrackableReader, DidntRead, HasReader, Reader};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{
    any::Any,
    convert::AsRef,
    fmt,
    num::NonZeroUsize,
    ops::{Deref, Index, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive},
};

/*************************************/
/*           ZSLICE BUFFER           */
/*************************************/
pub trait ZSliceBuffer: AsRef<[u8]> + AsMut<[u8]> + fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl ZSliceBuffer for Vec<u8> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ZSliceBuffer for Box<[u8]> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<const N: usize> ZSliceBuffer for [u8; N] {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/*************************************/
/*               ZSLICE              */
/*************************************/
/// A clonable wrapper to a contiguous slice of bytes.
#[derive(Clone)]
pub struct ZSlice {
    pub buf: Arc<dyn ZSliceBuffer>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl ZSlice {
    pub fn make(
        buf: Arc<dyn ZSliceBuffer>,
        start: usize,
        end: usize,
    ) -> Result<ZSlice, Arc<dyn ZSliceBuffer>> {
        if end <= buf.as_slice().len() {
            Ok(ZSlice { buf, start, end })
        } else {
            Err(buf)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf.as_slice()[self.start..self.end]
    }

    /// # Safety
    ///
    /// This function retrieves a mutable slice from a non-mutable reference.
    /// Mutating the content of the slice without proper syncrhonization is considered
    /// undefined behavior in Rust. To use with extreme caution.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut_slice(&self) -> &mut [u8] {
        let buf = unsafe { &mut (*(Arc::as_ptr(&self.buf) as *mut dyn ZSliceBuffer)) };
        &mut buf.as_mut_slice()[self.start..self.end]
    }

    pub(crate) fn new_sub_slice(&self, start: usize, end: usize) -> Option<ZSlice> {
        if end <= self.len() {
            Some(ZSlice {
                buf: self.buf.clone(),
                start: self.start + start,
                end: self.start + end,
            })
        } else {
            None
        }
    }
}

impl Deref for ZSlice {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for ZSlice {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl Index<usize> for ZSlice {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.buf.as_slice()[self.start + index]
    }
}

impl Index<Range<usize>> for ZSlice {
    type Output = [u8];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &(self.deref())[range]
    }
}

impl Index<RangeFrom<usize>> for ZSlice {
    type Output = [u8];

    fn index(&self, range: RangeFrom<usize>) -> &Self::Output {
        &(self.deref())[range]
    }
}

impl Index<RangeFull> for ZSlice {
    type Output = [u8];

    fn index(&self, _range: RangeFull) -> &Self::Output {
        self.deref()
    }
}

impl Index<RangeInclusive<usize>> for ZSlice {
    type Output = [u8];

    fn index(&self, range: RangeInclusive<usize>) -> &Self::Output {
        &(self.deref())[range]
    }
}

impl Index<RangeTo<usize>> for ZSlice {
    type Output = [u8];

    fn index(&self, range: RangeTo<usize>) -> &Self::Output {
        &(self.deref())[range]
    }
}

impl Index<RangeToInclusive<usize>> for ZSlice {
    type Output = [u8];

    fn index(&self, range: RangeToInclusive<usize>) -> &Self::Output {
        &(self.deref())[range]
    }
}

impl PartialEq for ZSlice {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for ZSlice {}

impl fmt::Display for ZSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02x?}", self.as_slice())
    }
}

impl fmt::Debug for ZSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZSlice{{ start: {}, end:{}, buf:\n {:02x?} \n}}",
            self.start,
            self.end,
            self.buf.as_slice()
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for ZSlice {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{:02x}", self.as_slice());
    }
}

// From impls
impl<T> From<Arc<T>> for ZSlice
where
    T: ZSliceBuffer + 'static,
{
    fn from(buf: Arc<T>) -> Self {
        let end = buf.as_slice().len();
        Self { buf, start: 0, end }
    }
}

impl<T> From<T> for ZSlice
where
    T: ZSliceBuffer + 'static,
{
    fn from(buf: T) -> Self {
        let end = buf.as_slice().len();
        Self {
            buf: Arc::new(buf),
            start: 0,
            end,
        }
    }
}

// Reader
impl HasReader for &mut ZSlice {
    type Reader = Self;

    fn reader(self) -> Self::Reader {
        self
    }
}

impl Reader for &mut ZSlice {
    fn read(&mut self, into: &mut [u8]) -> Result<NonZeroUsize, DidntRead> {
        let mut reader = self.as_slice().reader();
        let len = reader.read(into)?;
        self.start += len.get();
        Ok(len)
    }

    fn read_exact(&mut self, into: &mut [u8]) -> Result<(), DidntRead> {
        let mut reader = self.as_slice().reader();
        reader.read_exact(into)?;
        self.start += into.len();
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, DidntRead> {
        let mut reader = self.as_slice().reader();
        let res = reader.read_u8()?;
        self.start += 1;
        Ok(res)
    }

    fn read_zslices<F: FnMut(ZSlice)>(&mut self, len: usize, mut f: F) -> Result<(), DidntRead> {
        let zslice = self.read_zslice(len)?;
        f(zslice);
        Ok(())
    }

    fn read_zslice(&mut self, len: usize) -> Result<ZSlice, DidntRead> {
        let res = self.new_sub_slice(0, len).ok_or(DidntRead)?;
        self.start += len;
        Ok(res)
    }

    fn remaining(&self) -> usize {
        self.len()
    }

    fn can_read(&self) -> bool {
        !self.is_empty()
    }
}

impl BacktrackableReader for &mut ZSlice {
    type Mark = usize;

    fn mark(&mut self) -> Self::Mark {
        self.start
    }

    fn rewind(&mut self, mark: Self::Mark) -> bool {
        self.start = mark;
        true
    }
}

impl ZSlice {
    #[cfg(feature = "test")]
    pub fn rand(len: usize) -> Self {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        (0..len)
            .into_iter()
            .map(|_| rng.gen())
            .collect::<Vec<u8>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zslice() {
        let buf = crate::vec::uninit(16);
        let zslice: ZSlice = buf.clone().into();
        assert_eq!(buf.as_slice(), zslice.as_slice());

        let buf = (0..16).into_iter().collect::<Vec<u8>>();
        unsafe {
            let mbuf = zslice.as_mut_slice();
            mbuf[..buf.len()].clone_from_slice(&buf[..]);
        }
        assert_eq!(buf.as_slice(), zslice.as_slice());
    }
}
