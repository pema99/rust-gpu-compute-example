#![no_std]

use bytemuck_derive::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct TestVec {
    pub a: u32,
    pub b: u32
}