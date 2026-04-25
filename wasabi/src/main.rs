#![no_std] // 標準ライブラリを使わない（ベアメタル環境にはOSがないため）
#![no_main] // 通常のmain関数エントリポイントを使わない（UEFIのefi_mainを使うため）
#![feature(offset_of)] // offset_of!マクロを有効化（nightly限定機能、構造体レイアウト検証に使用）

use core::arch::asm; // インラインアセンブリ（hlt命令で使用）
use core::cmp::min; // 最小値を返す関数（is_in_x_rangeで使用）
use core::fmt;
use core::fmt::Write;
use core::mem::offset_of; // 構造体フィールドのオフセットを取得するマクロ（EFIテーブルのレイアウト検証で使用）
use core::mem::size_of; // 型のバイトサイズを取得する関数（EFI構造体のサイズ検証で使用）
use core::panic::PanicInfo; // パニックハンドラの引数型（#[panic_handler]で使用）
use core::ptr::null_mut; // nullポインタを生成する関数（UEFIプロトコル取得時に使用）
                         //use core::slice; // スライス操作（現在未使用）

type EfiVoid = u8;
type EfiHandle = u64;
type Result<T> = core::result::Result<T, &'static str>;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct EfiGuid {
    pub data0: u32,
    pub data1: u16,
    pub data2: u16,
    pub data3: [u8; 8],
}

const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data0: 0x9042a9de,
    data1: 0x23dc,
    data2: 0x4a38,
    data3: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[must_use]
#[repr(u64)]
enum EfiStatus {
    Success = 0,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum EfiMemoryType {
    RESERVED = 0,
    LOADED_CODE,
    LOADED_DATA,
    BOOT_SERVICE_CODE,
    BOOT_SERVICE_DATA,
    RUNTIME_SERVICES_CODE,
    RUNTIME_SERVICES_DATA,
    CONVENTIONAL_MEMORY,
    UNUSABLE_MEMORY,
    ACPI_RECLAIM_MEMORY,
    ACPI_MEMORY_NVS,
    MEMORY_MAPPED_IO,
    MEMORY_MAPPED_IO_PORT_SPACE,
    PAL_CODE,
    PERSISTENT_MEMORY,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct EfiMemoryDescriptor {
    memory_type: EfiMemoryType,
    physical_start: u64,
    virtual_start: u64,
    number_of_pages: u64,
    attribute: u64,
}

const MEMORY_MAP_BUFFER_SIZE: usize = 0x8000;

struct MemoryMapHolder {
    memory_map_buffer: [u8; MEMORY_MAP_BUFFER_SIZE],
    memory_map_size: usize,
    map_key: usize,
    descriptor_size: usize,
    descriptor_version: u32,
}

struct MemoryMapIterator<'a> {
    map: &'a MemoryMapHolder,
    ofs: usize,
}

impl<'a> Iterator for MemoryMapIterator<'a> {
    type Item = &'a EfiMemoryDescriptor;

    /// メモリマップバッファ上の次のEfiMemoryDescriptorへの参照を返す。
    /// 末尾に達したらNone。descriptor_size単位でオフセットを進めるため、
    /// 構造体サイズではなくUEFIから受け取った実サイズを使う。
    fn next(&mut self) -> Option<&'a EfiMemoryDescriptor> {
        if self.ofs >= self.map.memory_map_size {
            None
        } else {
            let e: &EfiMemoryDescriptor = unsafe {
                &*(self.map.memory_map_buffer.as_ptr().add(self.ofs) as *const EfiMemoryDescriptor)
            };
            self.ofs += self.map.descriptor_size;
            Some(e)
        }
    }
}

impl MemoryMapHolder {
    /// MemoryMapHolderをゼロ初期化して生成する。
    /// memory_map_sizeにはバッファ全体のサイズを入れておき、
    /// get_memory_map呼び出し時にUEFIが実サイズで上書きする。
    pub const fn new() -> MemoryMapHolder {
        MemoryMapHolder {
            memory_map_buffer: [0; MEMORY_MAP_BUFFER_SIZE],
            memory_map_size: MEMORY_MAP_BUFFER_SIZE,
            map_key: 0,
            descriptor_size: 0,
            descriptor_version: 0,
        }
    }
    /// 保持しているメモリマップを先頭から走査するイテレータを返す。
    pub fn iter(&self) -> MemoryMapIterator {
        MemoryMapIterator { map: self, ofs: 0 }
    }
}

#[repr(C)]
struct EfiBootServicesTable {
    _reserved0: [u64; 7],
    get_memory_map: extern "win64" fn(
        memory_map_size: *mut usize,
        memory_map: *mut u8,
        map_key: *mut usize,
        descriptor_size: *mut usize,
        descriptor_version: *mut u32,
    ) -> EfiStatus,

    _reserved1: [u64; 21],
    exit_boot_services: extern "win64" fn(image_handle: EfiHandle, map_key: usize) -> EfiStatus,

    _reserved4: [u64; 10],
    locate_protocol: extern "win64" fn(
        protocol: *const EfiGuid,
        registration: *const EfiVoid,
        interface: *mut *mut EfiVoid,
    ) -> EfiStatus,
}

impl EfiBootServicesTable {
    /// UEFIブートサービスのGetMemoryMapを呼び出し、現在のメモリマップを取得する。
    /// 取得結果（サイズ、map_key、descriptor_size、descriptor_version）は引数のmapに書き込まれる。
    fn get_memory_map(&self, map: &mut MemoryMapHolder) -> EfiStatus {
        (self.get_memory_map)(
            &mut map.memory_map_size,
            map.memory_map_buffer.as_mut_ptr(),
            &mut map.map_key,
            &mut map.descriptor_size,
            &mut map.descriptor_version,
        )
    }
}

const _: () = assert!(offset_of!(EfiBootServicesTable, get_memory_map) == 56);
const _: () = assert!(offset_of!(EfiBootServicesTable, exit_boot_services) == 232);
const _: () = assert!(offset_of!(EfiBootServicesTable, locate_protocol) == 320);

#[repr(C)]
struct EfiSystemTable {
    _reserved0: [u64; 12],
    pub boot_services: &'static EfiBootServicesTable,
}

const _: () = assert!(offset_of!(EfiSystemTable, boot_services) == 96);

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutProtocol<'a> {
    reserved: [u64; 3],
    pub mode: &'a EfiGraphicsOutputProtocolMode<'a>, // 現在利用中の画面モードに対応する情報を格納
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolMode<'a> {
    pub max_mode: u32,
    pub mode: u32,
    pub info: &'a EfiGraphicsOutputProtocolPixelInfo,
    pub size_of_info: u64,
    pub frame_buffer_base: usize, // フレームバッファの開始アドレス
    pub frame_buffer_size: usize, // フレームバッファのバイト単位での大きさ
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolPixelInfo {
    version: u32,
    pub horizontal_resolution: u32, // 水平方向の画素数
    pub vertical_resolution: u32,   // 垂直方向の画素数
    _padding0: [u32; 5],
    pub pixels_per_scan_line: u32,
}

const _: () = assert!(size_of::<EfiGraphicsOutputProtocolPixelInfo>() == 36);

/// UEFIのGraphics Output Protocolを取得する
/// efi_system_table: UEFIシステムテーブルへの参照
/// 戻り値: 成功すればグラフィック出力プロトコルへの参照、失敗すればErr。
fn locate_graphic_protocol<'a>(
    efi_system_table: &EfiSystemTable,
) -> Result<&'a EfiGraphicsOutProtocol<'a>> {
    let mut graphic_output_protocol = null_mut::<EfiGraphicsOutProtocol>();
    let status = (efi_system_table.boot_services.locate_protocol)(
        &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
        null_mut::<EfiVoid>(),
        &mut graphic_output_protocol as *mut *mut EfiGraphicsOutProtocol as *mut *mut EfiVoid,
    );

    if status != EfiStatus::Success {
        return Err("Failed to locate graphics output protocol");
    }

    Ok(unsafe { &*graphic_output_protocol })
}

/// x86のHLT命令を実行し、割り込みが来るまでCPUを休止させる
pub fn hlt() {
    unsafe { asm!("hlt") }
}

/// UEFIエントリポイント。VRAMを初期化し、矩形・直線・文字列の描画テストを行う。
#[no_mangle]
fn efi_main(image_handle: EfiHandle, efi_system_table: &EfiSystemTable) {
    let mut vram = init_vram(efi_system_table).expect("init_vram failed");

    let vw = vram.width;
    let vh = vram.height;

    fill_rect(&mut vram, 0x000000, 0, 0, vw, vh).expect("fill_rect failed");

    draw_test_pattern(&mut vram);

    let mut w = VramTextWriter::new(&mut vram);

    for i in 0..4 {
        writeln!(w, "i = {i}").unwrap()
    }

    let mut memory_map = MemoryMapHolder::new();
    let status = efi_system_table
        .boot_services
        .get_memory_map(&mut memory_map);

    writeln!(w, "{status:?}").unwrap();

    let mut total_memory_pages = 0;

    for e in memory_map.iter() {
        if e.memory_type != EfiMemoryType::CONVENTIONAL_MEMORY {
            continue;
        }

        total_memory_pages += e.number_of_pages;

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

/// パニック時のハンドラ。HLTループでCPUを停止させる。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        hlt()
    }
}

/// ピクセルバッファへの描画操作を抽象化するtrait。
/// traitはRustにおける「インターフェース」。具体的なデータ構造（VramBufferInfoなど）が
/// このtraitをimplすることで、draw_pointやfill_rectなどの描画関数を共通で使えるようになる。
/// 必須メソッド: bytes_per_pixel, pixels_per_line, width, height, buf_mut
/// デフォルト実装: unchecked_pixel_at_mut, pixel_at_mut, is_in_x_range, is_in_y_range
trait Bitmap {
    fn bytes_per_pixel(&self) -> i64;
    fn pixels_per_line(&self) -> i64;
    fn width(&self) -> i64;
    fn height(&self) -> i64;
    fn buf_mut(&mut self) -> *mut u8;

    unsafe fn unchecked_pixel_at_mut(&mut self, x: i64, y: i64) -> *mut u32 {
        self.buf_mut()
            .add(((y * self.pixels_per_line() + x) * self.bytes_per_pixel()) as usize)
            as *mut u32
    }

    fn pixel_at_mut(&mut self, x: i64, y: i64) -> Option<&mut u32> {
        if self.is_in_x_range(x) && self.is_in_y_range(y) {
            unsafe { Some(&mut *(self.unchecked_pixel_at_mut(x, y))) }
        } else {
            None
        }
    }

    fn is_in_x_range(&self, px: i64) -> bool {
        0 <= px && px < min(self.width(), self.pixels_per_line())
    }

    fn is_in_y_range(&self, py: i64) -> bool {
        0 <= py && py < self.height()
    }
}

#[derive(Clone, Copy)]
struct VramBufferInfo {
    buf: *mut u8,
    width: i64,
    height: i64,
    pixels_per_line: i64,
}

impl Bitmap for VramBufferInfo {
    /// 1ピクセルあたりのバイト数。UEFIのBlt系で標準的なBGRX 32bit想定なので常に4。
    fn bytes_per_pixel(&self) -> i64 {
        4
    }

    /// 1ライン（走査線）あたりのピクセル数。可視幅(width)と一致するとは限らず、
    /// 行末にパディングがある場合はwidthより大きい値になる（VRAMの行ストライド）。
    fn pixels_per_line(&self) -> i64 {
        self.pixels_per_line
    }

    /// 画面の水平方向の可視ピクセル数。
    fn width(&self) -> i64 {
        self.width
    }

    /// 画面の垂直方向の可視ピクセル数。
    fn height(&self) -> i64 {
        self.height
    }

    /// フレームバッファ先頭への可変ポインタ。生ポインタなのでアクセスはunsafe。
    fn buf_mut(&mut self) -> *mut u8 {
        self.buf
    }
}

/// UEFIのGraphics Output Protocolからフレームバッファ情報を取得し、VramBufferInfoを初期化する
/// efi_system_table: UEFIシステムテーブルへの参照
/// 戻り値: 成功すればVRAMのバッファ情報、失敗すればErr。
fn init_vram(efi_system_table: &EfiSystemTable) -> Result<VramBufferInfo> {
    let gp = locate_graphic_protocol(efi_system_table)?;
    Ok(VramBufferInfo {
        buf: gp.mode.frame_buffer_base as *mut u8,
        width: gp.mode.info.horizontal_resolution as i64,
        height: gp.mode.info.vertical_resolution as i64,
        pixels_per_line: gp.mode.info.pixels_per_scan_line as i64,
    })
}

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

/// 指定した位置とサイズで矩形を塗りつぶす
/// px, py: 左上の座標、w: 幅、h: 高さ
/// 範囲外ならErr、成功ならOk(())。範囲チェック後はunchecked版で高速に描画する。
fn fill_rect<T: Bitmap>(buf: &mut T, color: u32, px: i64, py: i64, w: i64, h: i64) -> Result<()> {
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

/// font.txtから指定した文字のフォントデータを検索する
/// c: 検索対象の文字（ASCII範囲）
/// 戻り値: 見つかれば8x16のフォントビットマップをSomeで返す。見つからなければNone。
/// font.txtは"0xXX"行でASCIIコードを示し、続く16行が8文字幅のドットパターン。
fn lookup_font(c: char) -> Option<[[char; 8]; 16]> {
    const FONT_SOURCE: &str = include_str!("./font.txt");

    if let Ok(c) = u8::try_from(c) {
        let mut fi = FONT_SOURCE.split('\n');
        while let Some(line) = fi.next() {
            if let Some(line) = line.strip_prefix("0x") {
                if let Ok(idx) = u8::from_str_radix(line, 16) {
                    if idx != c {
                        continue;
                    }
                    let mut font = [['*'; 8]; 16];
                    for (y, line) in fi.clone().take(16).enumerate() {
                        for (x, c) in line.chars().enumerate() {
                            if let Some(e) = font[y].get_mut(x) {
                                *e = c
                            }
                        }
                    }
                    return Some(font);
                }
            }
        }
    }
    None
}

/// 1文字を前景色のみで描画する
/// buf: 描画先のビットマップ
/// x, y: 描画開始位置（左上）の座標
/// color: 前景色（'*'のピクセルに使う色）
/// c: 描画する文字
/// フォントデータの'*'部分だけをcolorで描画し、それ以外はスキップ（背景は塗らない）。
fn draw_font_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, c: char) {
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
fn draw_str_fg<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32, s: &str) {
    for (i, c) in s.chars().enumerate() {
        // i文字目を 8 * i ピクセル分右にずらして描画
        draw_font_fg(buf, x + i as i64 * 8, y, color, c)
    }
}

struct VramTextWriter<'a> {
    vram: &'a mut VramBufferInfo,
    cursor_x: i64,
    cursor_y: i64,
}

impl<'a> VramTextWriter<'a> {
    /// 描画先のVRAMを受け取り、カーソルを左上(0, 0)に置いた状態で生成する。
    fn new(vram: &'a mut VramBufferInfo) -> Self {
        Self {
            vram,
            cursor_x: 0,
            cursor_y: 0,
        }
    }
}

impl fmt::Write for VramTextWriter<'_> {
    /// 文字列をVRAM上にカーソル位置から描画する（write!/writeln!マクロから呼ばれる）。
    /// '\n'なら改行（カーソルを左端に戻し、yを16px進める）。
    /// それ以外は白色で1文字描画し、カーソルを8px右へ進める。
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if c == '\n' {
                self.cursor_y += 16;
                self.cursor_x = 0;
                continue;
            }
            draw_font_fg(self.vram, self.cursor_x, self.cursor_y, 0xffffff, c);
            self.cursor_x += 8
        }
        Ok(())
    }
}

fn exit_from_efi_boot_services(
    image_handle: EfiHandle,
    efi_system_table: &EfiSystemTable,
    memory_map: &mut MemoryMapHolder,
) {
    loop {
        let status = efi_system_table.boot_services.get_memory_map(memory_map);

        assert_eq!(status, EfiStatus::Success);

        let status =
            (efi_system_table.boot_services.exit_boot_services)(image_handle, memory_map.map_key);

        if status == EfiStatus::Success {
            break;
        }
    }
}

/// 画面右側に描画機能の動作確認用テストパターンを描画する。
/// 4色の矩形と補色の矩形を縦に並べ、その上に四隅同士を結ぶ直線、
/// 下部に数字とアルファベットの文字列を表示する。
fn draw_test_pattern<T: Bitmap>(buf: &mut T) {
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
