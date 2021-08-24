#![no_std]

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TestVec {
    pub a: u32,
    pub b: u32,
    pub c: u32
}