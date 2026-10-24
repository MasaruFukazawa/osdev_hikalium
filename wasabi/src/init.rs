extern crate alloc;

use crate::allocator::ALLOCATOR;

use crate::uefi::exit_from_efi_boot_services;
use crate::uefi::EfiHandle;
use crate::uefi::EfiMemoryType::*;
use crate::uefi::EfiSystemTable;
use crate::uefi::MemoryMapHolder;
use crate::x86::write_cr3;
use crate::x86::PageAttr;
use crate::x86::PAGE_SIZE;
use crate::x86::PML4;
use alloc::boxed::Box;
use core::cmp::max;

/// OS の最低限のランタイム（メモリマップ取得＋ヒープアロケータ）を立ち上げる。
///
/// 流れ:
///   1. UEFI から現在のメモリマップを受け取りつつ ExitBootServices を呼んで
///      ファームウェア管理を抜ける（以降ファームウェアの BootServices は使えない）。
///   2. 受け取ったメモリマップの CONVENTIONAL_MEMORY 領域を `ALLOCATOR` に登録し、
///      `Box::new` 等のヒープ確保が使えるようにする。
///
/// 戻り値はそのメモリマップ。呼び出し側が領域種別の表示などに使う。
pub fn init_basic_runtime(
    image_handle: EfiHandle,
    efi_system_table: &EfiSystemTable,
) -> MemoryMapHolder {
    let mut memory_map = MemoryMapHolder::new();

    exit_from_efi_boot_services(image_handle, efi_system_table, &mut memory_map);

    ALLOCATOR.init_with_mmap(&memory_map);

    memory_map
}

pub fn init_paging(memory_map: &MemoryMapHolder) {
    let mut table = PML4::new();
    let mut end_of_mem = 0x1_0000_0000u64;

    for e in memory_map.iter() {
        match e.memory_type() {
            CONVENTIONAL_MEMORY | LOADED_CODE | LOADED_DATA => {
                end_of_mem = max(
                    end_of_mem,
                    e.physical_start() + e.number_of_pages() * (PAGE_SIZE as u64),
                )
            }
            _ => (),
        }
    }
    table
        .create_mapping(0, end_of_mem, 0, PageAttr::ReadWriteKernel)
        .expect("Failed to create initial page mapping");

    unsafe {
        write_cr3(Box::into_raw(table));
    }
}
