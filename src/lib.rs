#![feature(allocator_api)]
#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate libc_print;

pub mod allocator;
pub mod elementary;
