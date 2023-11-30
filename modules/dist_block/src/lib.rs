#![no_std]
#![feature(doc_auto_cfg)]
#![feature(const_trait_impl)]

extern crate alloc;

pub mod transport;
pub mod blk_device;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
    }
}
