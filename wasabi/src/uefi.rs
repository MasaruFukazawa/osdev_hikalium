use crate::graphics::draw_font_fg;
use crate::graphics::Bitmap;
use crate::result::Result;

use core::fmt;
use core::mem::offset_of;
use core::mem::size_of;
use core::ptr::null_mut;

// ---------------------------------------------------------------------------
// 型エイリアス
// ---------------------------------------------------------------------------

type EfiVoid = u8;
pub type EfiHandle = u64;

// ---------------------------------------------------------------------------
// EFI基本型
// ---------------------------------------------------------------------------

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
pub enum EfiStatus {
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
pub struct EfiMemoryDescriptor {
    memory_type: EfiMemoryType,
    physical_start: u64,
    virtual_start: u64,
    number_of_pages: u64,
    attribute: u64,
}

impl EfiMemoryDescriptor {
    pub fn memory_type(&self) -> EfiMemoryType {
        self.memory_type
    }

    pub fn number_of_pages(&self) -> u64 {
        self.number_of_pages
    }
}

// ---------------------------------------------------------------------------
// メモリマップ
// ---------------------------------------------------------------------------

const MEMORY_MAP_BUFFER_SIZE: usize = 0x8000;

pub struct MemoryMapHolder {
    memory_map_buffer: [u8; MEMORY_MAP_BUFFER_SIZE],
    memory_map_size: usize,
    map_key: usize,
    descriptor_size: usize,
    descriptor_version: u32,
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

impl Default for MemoryMapHolder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MemoryMapIterator<'a> {
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

// ---------------------------------------------------------------------------
// EFIテーブルとプロトコル
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct EfiBootServicesTable {
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
    pub fn get_memory_map(&self, map: &mut MemoryMapHolder) -> EfiStatus {
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
pub struct EfiSystemTable {
    _reserved0: [u64; 12],
    pub boot_services: &'static EfiBootServicesTable,
}

const _: () = assert!(offset_of!(EfiSystemTable, boot_services) == 96);

impl EfiSystemTable {
    pub fn boot_services(&self) -> &EfiBootServicesTable {
        self.boot_services
    }
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
struct EfiGraphicsOutProtocol<'a> {
    reserved: [u64; 3],
    pub mode: &'a EfiGraphicsOutputProtocolMode<'a>, // 現在利用中の画面モードに対応する情報を格納
}

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

// ---------------------------------------------------------------------------
// VRAMバッファ
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct VramBufferInfo {
    buf: *mut u8,
    pub width: i64,
    pub height: i64,
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
pub fn init_vram(efi_system_table: &EfiSystemTable) -> Result<VramBufferInfo> {
    let gp = locate_graphic_protocol(efi_system_table)?;
    Ok(VramBufferInfo {
        buf: gp.mode.frame_buffer_base as *mut u8,
        width: gp.mode.info.horizontal_resolution as i64,
        height: gp.mode.info.vertical_resolution as i64,
        pixels_per_line: gp.mode.info.pixels_per_scan_line as i64,
    })
}

// ---------------------------------------------------------------------------
// テキスト出力
// ---------------------------------------------------------------------------

pub struct VramTextWriter<'a> {
    vram: &'a mut VramBufferInfo,
    cursor_x: i64,
    cursor_y: i64,
}

impl<'a> VramTextWriter<'a> {
    /// 描画先のVRAMを受け取り、カーソルを左上(0, 0)に置いた状態で生成する。
    pub fn new(vram: &'a mut VramBufferInfo) -> Self {
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

/// UEFIブートサービスを終了し、OS側がメモリ等の制御を引き継ぐ。
/// image_handle: efi_mainに渡されたイメージハンドル。
/// efi_system_table: UEFIシステムテーブル。
/// memory_map: 直前に取得したメモリマップ。map_keyを使ってExitBootServicesを呼ぶ。
/// map_keyが古いと失敗するため、失敗したら最新のメモリマップを取り直してリトライする。
pub fn exit_from_efi_boot_services(
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
