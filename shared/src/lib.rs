#![no_std]

use spirv_std::glam::*;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TestVec {
    pub a: u32,
    pub b: u32,
    pub c: u32
}

#[repr(C)]
#[derive(Copy, Clone)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
pub struct Ray {
    pub origin: Vec4,
    pub direction: Vec4
}