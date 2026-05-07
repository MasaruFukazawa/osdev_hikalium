use crate::x86::hlt;
use crate::x86::write_io_port_u8;

/// `isa-debug-exit` デバイスに書き込んでホスト側 QEMU プロセスを終了させるための値。
/// QEMU の終了コードは `(value << 1) | 1` で計算されるので、Success=0x1 → 3、
/// Fail=0x2 → 5 がホスト（make やシェル）から見える QEMU の終了ステータスになる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x1, // QEMU will exit with status 3
    Fail = 0x2,    // QEMU will exit with status 5
}

/// QEMU を指定の終了コードで停止させる。`launch_qeme.sh` の
/// `-device isa-debug-exit,iobase=0x4,iosize=0x04` でマップされた I/O ポート 0x4 に
/// `exit_code` を書くと、QEMU は即座にプロセスを終了する。
/// 書き込み後に QEMU が消える前に CPU が暴走しないよう、無限 HLT で待機する。
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    write_io_port_u8(0x4, exit_code as u8);
    loop {
        hlt()
    }
}
