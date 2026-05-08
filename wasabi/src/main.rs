#![no_std] // 標準ライブラリを使わない（ベアメタル環境にはOSがないため）
#![no_main] // 通常のmain関数エントリポイントを使わない（UEFIのefi_mainを使うため）
#![feature(offset_of)] // offset_of!マクロを有効化（nightly限定機能、構造体レイアウト検証に使用）

use core::panic::PanicInfo; // パニックハンドラの引数型（#[panic_handler]で使用）

use wasabi::error;

use wasabi::graphics::draw_test_pattern;
use wasabi::graphics::fill_rect;
use wasabi::graphics::Bitmap;

use wasabi::info;

use wasabi::init::init_basic_runtime;

use wasabi::print::hexdump;
use wasabi::println;

use wasabi::qemu::exit_qemu;
use wasabi::qemu::QemuExitCode;

use wasabi::uefi::init_vram;
use wasabi::uefi::EfiHandle;
use wasabi::uefi::EfiMemoryType;
use wasabi::uefi::EfiSystemTable;

use wasabi::warn;

// ---------------------------------------------------------------------------
// ヘルパー関数
// ---------------------------------------------------------------------------

/// パニック時のハンドラ。HLTループでCPUを停止させる。
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {info:?}");
    exit_qemu(QemuExitCode::Fail);
}

// ---------------------------------------------------------------------------
// エントリポイント
// ---------------------------------------------------------------------------

/// UEFIエントリポイント。VRAMを初期化し、矩形・直線・文字列の描画テストを行う。
#[no_mangle]
fn efi_main(image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
    println!("Booting WasabiOS ... \n");
    println!("image_handle: {:#018X}\n", image_handle);
    println!("efi_system_table: {:#p}\n", efi_system_table);

    info!("info");
    warn!("warn");
    error!("error");
    hexdump(efi_system_table);

    let mut vram = init_vram(efi_system_table).expect("init_vram failed");
    let vw = vram.width();
    let vh = vram.height();

    fill_rect(&mut vram, 0x000000, 0, 0, vw, vh).expect("fill_rect failed");

    draw_test_pattern(&mut vram);

    let memory_map = init_basic_runtime(image_handle, efi_system_table);
    let mut total_memory_pages = 0;

    for e in memory_map.iter() {
        if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
            continue;
        }

        total_memory_pages += e.number_of_pages();

        println!("{e:?}");
    }

    let total_memory_size_mib = total_memory_pages * 4096 / 1024 / 1024;

    println!("Total: {total_memory_pages} pages = {total_memory_size_mib} Mib");
    println!("Hello, Non-UEFI world !");

    let cr3 = wasabi::x86::read_cr3();

    println!("cr3 = {cr3:#p}");
    hexdump(unsafe { &*cr3 });

    exit_qemu(QemuExitCode::Success);
}
