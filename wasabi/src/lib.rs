#![no_std]
#![feature(offset_of)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "run_unit_tests"]
#![no_main]
pub mod allocator;
pub mod graphics;
pub mod init;
pub mod print;
pub mod qemu;
pub mod result;
pub mod serial;
pub mod uefi;
pub mod x86;

#[cfg(test)]
pub mod test_runner;

/// `cargo test` でビルドした際のエントリポイント。
/// 通常ビルドの `efi_main`（main.rs）の代わりに、ランタイムを立ち上げてから
/// `#[test_case]` 群を実行する `run_unit_tests`（カスタムテストフレームワークが
/// 自動生成する関数、名前は `#![reexport_test_harness_main]` で指定）に飛ぶ。
#[cfg(test)]
#[no_mangle]
pub fn efi_main(image_handle: uefi::EfiHandle, efi_system_table: &uefi::EfiSystemTable) {
    init::init_basic_runtime(image_handle, efi_system_table);
    run_unit_tests()
}
