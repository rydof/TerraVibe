//! Tiny little-endian byte codec shared by the save formats (no dependencies).

use crate::inventory::ItemStack;
use crate::item::{ItemId, Prefix};

/// Truncated or malformed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
    pub fn stack(&mut self, s: &Option<ItemStack>) {
        match s {
            None => self.u8(0),
            Some(s) => {
                self.u8(1);
                self.u16(s.id.0);
                self.u32(s.count);
                self.u8(s.prefix.0);
            }
        }
    }
}

pub struct Reader<'a>(pub &'a [u8]);

impl Reader<'_> {
    pub fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        if self.0.len() < n {
            return Err(DecodeError);
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn stack(&mut self) -> Result<Option<ItemStack>, DecodeError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(ItemStack {
                id: ItemId(self.u16()?),
                count: self.u32()?,
                prefix: Prefix(self.u8()?),
            })),
            _ => Err(DecodeError),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
