use crate::result::Result;
use core::cmp::min;

// ---------------------------------------------------------------------------
// Bitmap trait（描画対象の抽象化）
// ---------------------------------------------------------------------------

/// ピクセルバッファへの描画操作を抽象化するtrait。
/// traitはRustにおける「インターフェース」。具体的なデータ構造（VramBufferInfoなど）が
/// このtraitをimplすることで、draw_pointやfill_rectなどの描画関数を共通で使えるようになる。
/// 必須メソッド: bytes_per_pixel, pixels_per_line, width, height, buf_mut
/// デフォルト実装: unchecked_pixel_at_mut, pixel_at_mut, is_in_x_range, is_in_y_range
pub trait Bitmap {
    /// 1ピクセルあたりのバイト数を返す（実装側で固定値、例: BGRX 32bitなら4）。
    fn bytes_per_pixel(&self) -> i64;
    /// 1ライン（走査線）あたりのピクセル数を返す。
    /// 可視幅(width)と一致するとは限らず、行末にパディングがある場合はwidthより大きい。
    fn pixels_per_line(&self) -> i64;
    /// 画面の水平方向の可視ピクセル数を返す。
    fn width(&self) -> i64;
    /// 画面の垂直方向の可視ピクセル数を返す。
    fn height(&self) -> i64;
    /// バッファ先頭への可変ポインタを返す。生ポインタなので呼び出し側でunsafeに扱う。
    fn buf_mut(&mut self) -> *mut u8;

    /// 範囲チェックなしで、指定座標のピクセルへの可変ポインタを返す。
    /// アドレスは buf + (y * pixels_per_line + x) * bytes_per_pixel で計算する。
    ///
    /// # Safety
    /// (x, y) が描画可能範囲内であることを呼び出し側で保証する必要がある。
    /// 範囲外の座標を指定するとバッファ外への書き込みになり未定義動作。
    unsafe fn unchecked_pixel_at_mut(&mut self, x: i64, y: i64) -> *mut u32 {
        self.buf_mut()
            .add(((y * self.pixels_per_line() + x) * self.bytes_per_pixel()) as usize)
            as *mut u32
    }

    /// 範囲チェック付きで、指定座標のピクセルへの可変参照を返す。
    /// 範囲外ならNone、範囲内ならSomeに包んで返す。
    fn pixel_at_mut(&mut self, x: i64, y: i64) -> Option<&mut u32> {
        if self.is_in_x_range(x) && self.is_in_y_range(y) {
            unsafe { Some(&mut *(self.unchecked_pixel_at_mut(x, y))) }
        } else {
            None
        }
    }

    /// 与えられたx座標が描画可能範囲（0以上、width/pixels_per_lineの小さい方未満）かを返す。
    fn is_in_x_range(&self, px: i64) -> bool {
        0 <= px && px < min(self.width(), self.pixels_per_line())
    }

    /// 与えられたy座標が描画可能範囲（0以上、height未満）かを返す。
    fn is_in_y_range(&self, py: i64) -> bool {
        0 <= py && py < self.height()
    }
}

// ---------------------------------------------------------------------------
// ピクセル描画（プリミティブ）
// ---------------------------------------------------------------------------

/// 範囲チェックなしで1ピクセルに色を書き込む（高速版、呼び出し側で範囲保証が必要）
unsafe fn unchecked_draw_point<T: Bitmap>(buf: &mut T, color: u32, x: i64, y: i64) {
    *buf.unchecked_pixel_at_mut(x, y) = color;
}

/// 範囲チェック付きで1ピクセルに色を書き込む
/// 座標が範囲外ならErr、成功ならOk(())。
fn draw_point<T: Bitmap>(buf: &mut T, color: u32, x: i64, y: i64) -> Result<()> {
    *(buf.pixel_at_mut(x, y).ok_or("Out of Range")?) = color;
    Ok(())
}

// ---------------------------------------------------------------------------
// 図形描画
// ---------------------------------------------------------------------------

/// 指定した位置とサイズで矩形を塗りつぶす
/// px, py: 左上の座標、w: 幅、h: 高さ
/// 範囲外ならErr、成功ならOk(())。範囲チェック後はunchecked版で高速に描画する。
pub fn fill_rect<T: Bitmap>(
    buf: &mut T,
    color: u32,
    px: i64,
    py: i64,
    w: i64,
    h: i64,
) -> Result<()> {
    if !buf.is_in_x_range(px)
        || !buf.is_in_y_range(py)
        || !buf.is_in_x_range(px + w - 1)
        || !buf.is_in_y_range(py + h - 1)
    {
        return Err("Out of Range");
    }

    for y in py..py + h {
        for x in px..px + w {
            unsafe { unchecked_draw_point(buf, color, x, y) }
        }
    }

    Ok(())
}

/// 直線描画用の傾き計算。主軸方向の距離da、副軸方向の距離db、主軸上の現在位置iaから副軸の座標を求める。
/// da < dbなら描画不要（None）、それ以外は整数除算で副軸の位置をSomeで返す。
fn calc_slope_point(da: i64, db: i64, ia: i64) -> Option<i64> {
    if da < db {
        None
    } else if da == 0 {
        Some(0)
    } else if (0..=da).contains(&ia) {
        Some((2 * db * ia + da) / da / 2)
    } else {
        None
    }
}

/// 2点間に直線を描画する（ブレゼンハム方式）
/// buf: 描画先のビットマップ
/// color: 線の色
/// x0, y0: 始点の座標
/// x1, y1: 終点の座標
/// 戻り値: 座標が範囲外ならErr、成功ならOk(())。
/// dx >= dy なら水平寄り、そうでなければ垂直寄りに軸を切り替えて描画する。
fn draw_line<T: Bitmap>(buf: &mut T, color: u32, x0: i64, y0: i64, x1: i64, y1: i64) -> Result<()> {
    if !buf.is_in_x_range(x0)
        || !buf.is_in_x_range(x1)
        || !buf.is_in_y_range(y0)
        || !buf.is_in_y_range(y1)
    {
        return Err("Out of Range");
    }

    let dx = (x1 - x0).abs();
    let sx = (x1 - x0).signum();
    let dy = (y1 - y0).abs();
    let sy = (y1 - y0).signum();

    if dx >= dy {
        for (rx, ry) in (0..dx).flat_map(|rx| calc_slope_point(dx, dy, rx).map(|ry| (rx, ry))) {
            draw_point(buf, color, x0 + rx * sx, y0 + ry * sy)?;
        }
    } else {
        for (rx, ry) in (0..dy).flat_map(|ry| calc_slope_point(dy, dx, ry).map(|rx| (rx, ry))) {
            draw_point(buf, color, x0 + rx * sx, y0 + ry * sy)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 文字描画
// ---------------------------------------------------------------------------

/// font.txtから指定した文字のフォントデータを検索する
/// c: 検索対象の文字（ASCII範囲）
/// 戻り値: 見つかれば8x16のフォントビットマップをSomeで返す。見つからなければNone。
/// font.txtは"0xXX"行でASCIIコードを示し、続く16行が8文字幅のドットパターン。
fn lookup_font(c: char) -> Option<[[char; 8]; 16]> {
    const FONT_SOURCE: &str = include_str!("./font.txt");
    static mut FONT_CACHE: Option<[[[char; 8]; 16]; 256]> = None;

    if let Ok(c) = u8::try_from(c) {
        let font = unsafe {
            FONT_CACHE.get_or_insert_with(|| {
                let mut font = [[['*'; 8]; 16]; 256];
                let mut fi = FONT_SOURCE.split('\n');

                while let Some(line) = fi.next() {
                    if let Some(line) = line.strip_prefix("0x") {
                        if let Ok(idx) = u8::from_str_radix(line, 16) {
                            let mut glyph = [['*'; 8]; 16];
                            for (y, line) in fi.clone().take(16).enumerate() {
                                for (x, c) in line.chars().enumerate() {
                                    if let Some(e) = glyph[y].get_mut(x) {
                                        *e = c;
                                    }
                                }
                            }
                            font[idx as usize] = glyph;
                        }
                    }
                }
                font
            })
        };
        Some(font[c as usize])
    } else {
        None
    }
}

/// 1文字を前景色のみで描画する
/// buf: 描画先のビットマップ
/// x, y: 描画開始位置（左上）の座標
/// color: 前景色（'*'のピクセルに使う色）
/// c: 描画する文字
/// フォントデータの'*'部分だけをcolorで描画し、それ以外はスキップ（背景は塗らない）。
pub fn draw_font_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, c: char) {
    if let Some(font) = lookup_font(c) {
        for (dy, row) in font.iter().enumerate() {
            for (dx, pixel) in row.iter().enumerate() {
                let color = match pixel {
                    '*' => color,
                    _ => continue,
                };
                let _ = draw_point(buf, color, x + dx as i64, y + dy as i64);
            }
        }
    }
}

/// 文字列を前景色のみで描画する
/// buf: 描画先のビットマップ
/// x, y: 描画開始位置（左上）の座標
/// color: 前景色（文字の色）
/// s: 描画する文字列
/// 戻り値: なし。1文字あたり8px幅で横に並べて描画する。
pub fn draw_str_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, s: &str) {
    for (i, c) in s.chars().enumerate() {
        // i文字目を 8 * i ピクセル分右にずらして描画
        draw_font_fg(buf, x + i as i64 * 8, y, color, c)
    }
}

// ---------------------------------------------------------------------------
// テストパターン
// ---------------------------------------------------------------------------

/// 画面右側に描画機能の動作確認用テストパターンを描画する。
/// 4色の矩形と補色の矩形を縦に並べ、その上に四隅同士を結ぶ直線、
/// 下部に数字とアルファベットの文字列を表示する。
pub fn draw_test_pattern<T: Bitmap>(buf: &mut T) {
    let w = 128;
    let h = 64;

    let left = buf.width() - w - 1;
    let colors = [0x000000, 0xff0000, 0x00ff00, 0x0000ff];

    for (i, c) in colors.iter().enumerate() {
        let y = i as i64 * h;
        fill_rect(buf, *c, left, y, h, h).expect("fill_rect failed");
        fill_rect(buf, !*c, left + h, y, h, h).expect("fill_rect failed")
    }

    let points = [(0, 0), (0, w), (w, 0), (w, w)];

    for (x0, y0) in points.iter() {
        for (x1, y1) in points.iter() {
            let _ = draw_line(buf, 0xffffff, left + *x0, *y0, left + *x1, *y1);
        }
    }

    draw_str_fg(buf, left, h * colors.len() as i64, 0x00ff00, "0123456789");
    draw_str_fg(buf, left, h * colors.len() as i64 + 16, 0x00ff00, "ABCDEF");
}
