use crate::allocator::ALLOCATOR;

use crate::uefi::exit_from_efi_boot_services;
use crate::uefi::EfiHandle;
use crate::uefi::EfiSystemTable;
use crate::uefi::MemoryMapHolder;

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
