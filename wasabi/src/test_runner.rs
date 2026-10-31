use crate::qemu::exit_qemu;
use crate::qemu::QemuExitCode;

use crate::serial::SerialPort;

use core::any::type_name;

use core::fmt::Write;

use core::panic::PanicInfo;

/// テスト関数を「実行可能なもの」として抽象化するトレイト。
/// `test_runner` が `&[&dyn Testable]` で受け取れるよう、
/// 個々のテストを動的ディスパッチ越しに扱うために用意している。
pub trait Testable {
    /// このテストを 1 つ実行する。`writer` は実行ログ（実行中／成功）を
    /// シリアル経由でホスト側に流すための出力先。
    fn run(&self, writer: &mut SerialPort);
}

/// 引数を取らず値を返さない `Fn()` を全て `Testable` として扱えるブランケット実装。
/// `#[test_case]` を付けた通常の関数は `fn()`（関数ポインタ）として収集され、
/// `fn` は `Fn` を実装するためここに当てはまる。
/// 実行前後に `[RUNNING]` / `[PASS]` のログを書き、本体は `self()` で呼び出す。
/// 本体が panic した場合はこの関数を抜けずに `#[panic_handler]` 側へ飛ぶので、
/// `[PASS]` は出力されない（= 失敗が観測できる）。
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self, writer: &mut SerialPort) {
        writeln!(writer, "[RUNNING] >>> {}", type_name::<T>()).unwrap();
        self();
        writeln!(writer, "[PASS] <<< {}", type_name::<T>()).unwrap();
    }
}

/// カスタムテストフレームワーク（`#![test_runner = "..."]`）のエントリ。
/// `#[test_case]` で集められたテスト群を順に実行し、全部完走したら
/// QEMU を Success コードで終了させる（ホスト側の make が PASS と判定）。
/// 1 つでも panic すれば `#[panic_handler]` 経由で Fail 終了するので、
/// この関数の末尾まで到達することが「全テスト成功」を意味する。
pub fn test_runner(tests: &[&dyn Testable]) -> ! {
    let mut sw = SerialPort::new_for_com1();

    writeln!(sw, "Running {} tests...", tests.len()).unwrap();

    for test in tests {
        test.run(&mut sw);
    }

    writeln!(sw, "Completed {} tests...", tests.len()).unwrap();

    exit_qemu(QemuExitCode::Success)
}

/// テストビルド時の panic ハンドラ。
/// `unimplemented!` / `assert_*` 失敗 / その他の panic を全部ここで受け、
/// panic 情報をシリアルに書き出した上で QEMU を Fail コードで終了させる。
/// `no_std` 環境では panic 時の挙動を自前で定義する必要があるため必須。
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut sw = SerialPort::new_for_com1();

    writeln!(sw, "PANIC during test: {info:?}").unwrap();

    exit_qemu(QemuExitCode::Fail)
}
