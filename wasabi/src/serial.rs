use crate::x86::busy_loop_hint;
use crate::x86::read_io_port_u8;
use crate::x86::write_io_port_u8;

use core::fmt;

// https://wiki.osdev.org/Serial_Ports

/// 16550A 互換 UART（PC 標準のシリアルポート）への薄いラッパ。
/// `base` は I/O ポートのベースアドレスで、データレジスタ＝base、
/// 各種制御／ステータスレジスタは base+1〜base+5 に並ぶ。
pub struct SerialPort {
    base: u16,
}

impl SerialPort {
    /// 任意の I/O ポートを指すシリアルポートインスタンスを作る。
    /// 実際のハードウェア初期化（ボーレート設定等）は `init` で行う。
    pub fn new(base: u16) -> Self {
        Self { base }
    }

    /// PC 標準の COM1（I/O ポート 0x3f8）を指すインスタンスを作る。
    /// QEMU はデフォルトで COM1 をホストの stdio やログファイルに繋ぐので、
    /// デバッグ出力先として最も使い勝手が良い。
    pub fn new_for_com1() -> Self {
        // Use COM1 at I/O port 0x3f8
        Self::new(0x3f8)
    }

    /// UART を 115200 baud / 8N1 / FIFO 有効で初期化する。
    /// DLAB を立ててボーレート除数を書く → 8N1 設定 → FIFO 有効化、の順。
    /// 起動時に一度だけ呼べばよい。
    pub fn init(&mut self) {
        // Disable all interrupts
        write_io_port_u8(self.base + 1, 0x00);

        // Enable DLAB (set baud rate divisor)
        write_io_port_u8(self.base + 3, 0x80);

        // baud rate = (115200 / BAUD_DIVISOR)
        const BAUD_DIVISOR: u16 = 0x0001;
        write_io_port_u8(self.base, (BAUD_DIVISOR & 0xff) as u8);
        write_io_port_u8(self.base + 1, (BAUD_DIVISOR >> 8) as u8);

        // 8 bits, no parity, one stop bit
        write_io_port_u8(self.base + 3, 0x03);

        // Enable FIFO, clear them, with 14-byte threshold
        write_io_port_u8(self.base + 2, 0xC7);

        // IRQs enablem RTS/DSE set
        write_io_port_u8(self.base + 4, 0x08);
    }

    /// 1 文字をシリアルへ送信する。送信レジスタが空（LSR の THRE = bit5）になるまで
    /// `pause` を挟みつつ待ってから書き込む。マルチバイト文字（非 ASCII）は
    /// 下位 8bit にトランケートされるので注意。
    pub fn send_char(&self, c: char) {
        while (read_io_port_u8(self.base + 5) & 0x20) == 0 {
            busy_loop_hint();
        }
        write_io_port_u8(self.base, c as u8);
    }

    /// 文字列を 1 文字ずつ `send_char` で送る。文字数を先に数えてループしているのは
    /// `Iterator::next` を `unwrap` で取り出す愚直な実装にしているため。
    pub fn send_str(&self, s: &str) {
        let mut sc = s.chars();
        let slen = s.chars().count();

        for _ in 0..slen {
            self.send_char(sc.next().unwrap());
        }
    }
}

impl fmt::Write for SerialPort {
    /// `write!` / `writeln!` から呼ばれるエントリ。
    /// `print!` / `println!` マクロが `global_print` 経由でこのトレイトメソッドを使う。
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let serial = Self::default();
        serial.send_str(s);
        Ok(())
    }
}

impl Default for SerialPort {
    /// 既定値は COM1 を指すインスタンス。`SerialPort::default()` 一発で
    /// すぐに送信可能なポートが手に入るので、デバッグ出力経路を簡潔に書ける。
    fn default() -> Self {
        Self::new_for_com1()
    }
}
