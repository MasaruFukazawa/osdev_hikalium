#![no_std] // 標準ライブラリを使わない（ベアメタル環境にはOSがないため）
#![no_main] // 通常のmain関数エントリポイントを使わない（UEFIのefi_mainを使うため）
#![feature(offset_of)] // offset_of!マクロを有効化（nightly限定機能、構造体レイアウト検証に使用）

use core::arch::asm; // インラインアセンブリ（hlt命令で使用）
use core::fmt::Write;
use core::panic::PanicInfo; // パニックハンドラの引数型（#[panic_handler]で使用）
use core::writeln;

use wasabi::graphics::draw_test_pattern;
use wasabi::graphics::fill_rect;
use wasabi::graphics::Bitmap;

use wasabi::uefi::exit_from_efi_boot_services;
use wasabi::uefi::init_vram;
use wasabi::uefi::EfiHandle;
use wasabi::uefi::EfiMemoryType;
use wasabi::uefi::EfiSystemTable;
use wasabi::uefi::MemoryMapHolder;
use wasabi::uefi::VramTextWriter;

// ---------------------------------------------------------------------------
// ヘルパー関数
// ---------------------------------------------------------------------------

/// x86のHLT命令を実行し、割り込みが来るまでCPUを休止させる
pub fn hlt() {
    unsafe { asm!("hlt") }
}

/// パニック時のハンドラ。HLTループでCPUを停止させる。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        hlt()
    }
}

// ---------------------------------------------------------------------------
// エントリポイント
// ---------------------------------------------------------------------------

/// UEFIエントリポイント。VRAMを初期化し、矩形・直線・文字列の描画テストを行う。
#[no_mangle]
fn efi_main(image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
    let mut vram = init_vram(efi_system_table).expect("init_vram failed");

    let vw = vram.width();
    let vh = vram.height();

    fill_rect(&mut vram, 0x000000, 0, 0, vw, vh).expect("fill_rect failed");

    draw_test_pattern(&mut vram);

    let mut w = VramTextWriter::new(&mut vram);

    for i in 0..4 {
        writeln!(w, "i = {i}").unwrap()
    }

    let mut memory_map = MemoryMapHolder::new();
    let status = efi_system_table
        .boot_services()
        .get_memory_map(&mut memory_map);

    writeln!(w, "{status:?}").unwrap();

    let mut total_memory_pages = 0;

    for e in memory_map.iter() {
        if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
            continue;
        }

        total_memory_pages += e.number_of_pages();

        writeln!(w, "{e:?}").unwrap();
    }

    let total_memory_size_mib = total_memory_pages * 4096 / 1024 / 1024;

    writeln!(
        w,
        "Total: {total_memory_pages} pages = {total_memory_size_mib} Mib"
    )
    .unwrap();

    exit_from_efi_boot_services(image_handle, efi_system_table, &mut memory_map);

    writeln!(w, "Hello, Non-UEFI world !").unwrap();

    loop {
        hlt()
    }
}
