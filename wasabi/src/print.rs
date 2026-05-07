use crate::serial::SerialPort;

use core::fmt;
use core::mem::size_of;
use core::slice;

/// `print!` 等の各マクロから呼ばれる出力本体。
/// `format_args!` で組み立てた `fmt::Arguments` を、毎回 COM1 用の `SerialPort` を
/// 生成して書き出す（保持コストが小さく、グローバルロックを避けるための割り切り）。
pub fn global_print(args: fmt::Arguments) {
    let mut writer = SerialPort::default();
    fmt::write(&mut writer, args).unwrap();
}

/// シリアルへ整形出力する基本マクロ。改行は付与しない。
/// `wasabi::print!("...")` の形で外部クレートからも使える（`#[macro_export]` のため）。
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::global_print(format_args!($($arg)*)));
}

/// `print!` の末尾改行付き版。`std` の `println!` と同等の使い勝手を提供する。
#[macro_export]
macro_rules! println {
    () => ($crate::print("\n"));
        ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// 情報レベルのログマクロ。ファイル名・行番号・本文を `[INFO] ...` の体裁で出力する。
/// 呼び出し場所を素早く特定したいデバッグ時に使う。
#[macro_export]
macro_rules! info {
    () => ($crate::print("\n"));
        ($($arg:tt)*) => (
            $crate::print!("[INFO] {}:{:<3}: {}\n",
            file!(),
            line!(),
            format_args!($($arg)*))
        );
}

/// 警告レベルのログマクロ。`info!` と同じ書式で `[WARN]` プレフィクスを付ける。
#[macro_export]
macro_rules! warn {
    () => ($crate::print("\n"));
        ($($arg:tt)*) => (
            $crate::print!("[WARN] {}:{:<3}: {}\n",
            file!(),
            line!(),
            format_args!($($arg)*))
        );
}

/// エラーレベルのログマクロ。`info!` と同じ書式で `[ERROR]` プレフィクスを付ける。
/// パニックハンドラから呼んで停止前に状況を残す用途にも使える。
#[macro_export]
macro_rules! error {
    () => ($crate::print("\n"));
        ($($arg:tt)*) => (
            $crate::print!("[ERROR] {}:{:<3}: {}\n",
            file!(),
            line!(),
            format_args!($($arg)*))
        );
}

/// バイト列を 16 バイト幅でアドレス付きの hex + ASCII にダンプする想定の関数。
/// 注意: 現状は本体未実装のスタブで、ループは何も出力しない。
fn hexdump_bytes(bytes: &[u8]) {
    let mut i = 0;
    let mut ascii = [0x8; 16];
    let mut offset = 0;

    for v in bytes.iter() {
        if i == 0 {
            print!("{offset:08X}: ");
        }

        print!("{:02X} ", v);

        ascii[i] = *v;

        i += 1;

        if i == 16 {
            print!("|");

            for c in ascii.iter() {
                print!(
                    "{}",
                    match c {
                        0x20..=0x7e => {
                            *c as char
                        }
                        _ => {
                            '.'
                        }
                    }
                );
            }

            print!("|");

            offset += 16;

            i = 0;
        }
    }

    if i != 0 {
        let old_i = i;

        while i < 16 {
            print!("   ");
            i += 1;
        }

        print!("|");

        for c in ascii[0..old_i].iter() {
            print!(
                "{}",
                if (0x20u8..=0x7fu8).contains(c) {
                    *c as char
                } else {
                    '.'
                }
            );
        }

        print!("|");
    }
}

/// 任意の `Sized` 型の値を、その先頭から `size_of::<T>()` バイト分ダンプする。
/// 内部で生ポインタからスライスを作って `hexdump_bytes` に渡す。
///
/// # Safety
/// `T` は `repr(C)` 等でレイアウトが安定していることが望ましい。
/// パディング領域の値も読まれるので、未初期化バイトを含む型には使わないこと。
pub fn hexdump<T: Sized>(data: &T) {
    hexdump_bytes(unsafe { slice::from_raw_parts(data as *const T as *const u8, size_of::<T>()) })
}
