use core::arch::asm;

/// x86のHLT命令を実行し、割り込みが来るまでCPUを休止させる。
/// 無限ループの待機や、停止後の暴走防止に使う。
pub fn hlt() {
    unsafe { asm!("hlt") }
}

/// `pause` 命令を発行し、スピンロック中であることを CPU に伝える。
/// 投機的実行を抑え消費電力とメモリ順序ハザードを軽減するためのヒントで、
/// busy-wait ループ（シリアル送信待ちなど）に挟むと効率が良い。
pub fn busy_loop_hint() {
    unsafe { asm!("pause") }
}

/// 指定した I/O ポートから 1 バイトを読み取って返す（`in al, dx`）。
/// シリアル UART のステータスレジスタ等、メモリではなく I/O 空間にある
/// デバイスレジスタへのアクセスに使う。
pub fn read_io_port_u8(port: u16) -> u8 {
    let mut data: u8;

    unsafe {
        asm!(
            "in al, dx",
            out("al") data,
            in("dx") port
        )
    }

    data
}

/// 指定した I/O ポートに 1 バイトを書き込む（`out dx, al`）。
/// シリアル UART の制御や、QEMU の `isa-debug-exit` への終了コード書き込み等に使う。
pub fn write_io_port_u8(port: u16, data: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("al") data,
            in("dx") port
        )
    }
}

pub fn read_cr3() -> *mut RootPageTable {
    let mut cr3: *mut RootPageTable;

    unsafe {
        asm!(
            "mov rax, cr3",
            out("rax") cr3
        )
    }

    cr3
}

pub type RootPageTable = [u8; 1024];
