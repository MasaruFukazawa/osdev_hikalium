extern crate alloc;

use crate::result::Result;

use crate::uefi::EfiMemoryDescriptor;
use crate::uefi::EfiMemoryType;
use crate::uefi::MemoryMapHolder;

use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use alloc::boxed::Box;

use core::borrow::BorrowMut;
use core::cell::RefCell;
use core::cmp::max;
use core::fmt;
use core::mem::size_of;
use core::ops::DerefMut;
use core::ptr::null_mut;

/// 引数 v 以上の最小の2のべき乗を返す。
/// 例: 1→1, 2→2, 3→4, 5→8, 1000→1024。
/// (v-1)の先行ゼロ数から必要なシフト量を求めて 1<<n を計算する。
/// usize::MAX 近くの値で 1<<usize::BITS が起きるとオーバーフローするので、
/// checked_shl で None になった場合は "Out of range" を返す。
pub fn round_up_to_nearest_pow2(v: usize) -> Result<usize> {
    1usize
        .checked_shl(usize::BITS - v.wrapping_sub(1).leading_zeros())
        .ok_or("Out of range")
}

#[test_case]
fn round_up_to_nearest_pow2_tests() {
    //unimplemented!("cargo test should fail, right...?");
    assert_eq!(round_up_to_nearest_pow2(0), Err("Out of range"));
    assert_eq!(round_up_to_nearest_pow2(1), Ok(1));
    assert_eq!(round_up_to_nearest_pow2(2), Ok(2));
    assert_eq!(round_up_to_nearest_pow2(3), Ok(4));
    assert_eq!(round_up_to_nearest_pow2(4), Ok(4));
    assert_eq!(round_up_to_nearest_pow2(5), Ok(8));
    assert_eq!(round_up_to_nearest_pow2(6), Ok(8));
    assert_eq!(round_up_to_nearest_pow2(7), Ok(8));
    assert_eq!(round_up_to_nearest_pow2(8), Ok(8));
    assert_eq!(round_up_to_nearest_pow2(9), Ok(16));
}

/// 連結リスト方式のヒープ管理用ヘッダ。
/// 各メモリ領域の先頭に配置され、自分が管理する領域全体のサイズと
/// 割り当て状態、次の領域の Header への所有権を保持する。
/// next_header を Box で持つことで、リンクの解放が領域の解放を兼ねる。
struct Header {
    next_header: Option<Box<Header>>,
    size: usize,
    is_allocated: bool,
    _reserved: usize,
}

const HEADER_SIZE: usize = size_of::<Header>();

#[allow(clippy::assertions_on_constants)]
const _: () = assert!(HEADER_SIZE == 32);

// size of Header should be power of 2
const _: () = assert!(HEADER_SIZE.count_ones() == 1);

pub const LAYOUT_PAGE_4K: Layout = unsafe { Layout::from_size_align_unchecked(4096, 4096) };

impl Header {
    /// この空き領域から指定サイズ＋アライメントの割り当てが可能かを判定する。
    /// 必要量は「要求サイズ + ヘッダ2個分（自分用と分割後の残り用） + アライン余白」。
    fn can_provide(&self, size: usize, align: usize) -> bool {
        self.size >= size + HEADER_SIZE * 2 + align
    }

    /// このヘッダが管理する領域が現在割り当て済みかを返す。
    fn is_allocated(&self) -> bool {
        self.is_allocated
    }

    /// このヘッダが管理する領域の末尾アドレス（次の領域の開始アドレス）を返す。
    fn end_addr(&self) -> usize {
        self as *const Header as usize + self.size
    }

    /// 指定アドレスに新しい Header をゼロ初期化で書き込み、所有権付きの Box として返す。
    ///
    /// # Safety
    /// addr は Header を書き込み可能で、かつ HEADER_SIZE バイト以上の有効な領域の
    /// 先頭でなければならない。呼び出し側で重複した Box 化が起きないよう責任を持つ。
    unsafe fn new_from_addr(addr: usize) -> Box<Header> {
        let header = addr as *mut Header;
        header.write(Header {
            next_header: None,
            size: 0,
            is_allocated: false,
            _reserved: 0,
        });
        Box::from_raw(addr as *mut Header)
    }

    /// 割り当て済み領域の先頭ポインタから、その直前にある Header を Box として復元する。
    /// アロケータの dealloc 経路で使い、返した Box の Drop が連結リストの解放につながる。
    ///
    /// # Safety
    /// addr は本アロケータが alloc で返したポインタでなければならない。
    /// 他所で確保したポインタを渡すと未定義動作。
    unsafe fn from_allocated_region(addr: *mut u8) -> Box<Header> {
        let header = addr.sub(HEADER_SIZE) as *mut Header;
        Box::from_raw(header)
    }

    /// この空き領域から要求サイズ＋アラインを満たす領域を切り出し、その先頭ポインタを返す。
    /// 失敗時（既に割り当て済み、容量不足、サイズが2のべき乗化できない等）は None を返す。
    ///
    /// 切り出し戦略: 自分（self）の末尾側から領域を取る方式。
    /// レイアウト（アドレス昇順）は次のようになる:
    ///
    ///   [ self（縮小後の空き） | header_for_allocated | 要求領域 | header_for_padding | 末尾余白 ]
    ///
    /// - `header_for_allocated` は要求領域の直前に置かれ、is_allocated = true。
    /// - アライン調整で要求領域と self の末尾の間に隙間ができた場合のみ、
    ///   余白を管理する `header_for_padding`（is_allocated = false）を作って連結リストに挟む。
    /// - self.size は使用分だけ縮め、次のリンク先を `header_for_allocated` に張り替える。
    ///
    /// 戻り値は要求領域の先頭アドレス（呼び出し側から見たユーザ領域）。
    fn provide(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let size = max(round_up_to_nearest_pow2(size).ok()?, HEADER_SIZE);
        let align = max(align, HEADER_SIZE);

        if self.is_allocated() || !self.can_provide(size, align) {
            None
        } else {
            let mut size_used = 0;
            let allocated_addr = (self.end_addr() - size) & !(align - 1);
            let mut header_for_allocated =
                unsafe { Self::new_from_addr(allocated_addr - HEADER_SIZE) };

            header_for_allocated.is_allocated = true;
            header_for_allocated.size = size + HEADER_SIZE;
            size_used += header_for_allocated.size;
            header_for_allocated.next_header = self.next_header.take();

            if header_for_allocated.end_addr() != self.end_addr() {
                let mut header_for_padding =
                    unsafe { Self::new_from_addr(header_for_allocated.end_addr()) };

                header_for_padding.is_allocated = false;
                header_for_padding.size = self.end_addr() - header_for_allocated.end_addr();
                size_used += header_for_padding.size;
                header_for_padding.next_header = header_for_allocated.next_header.take();
                header_for_allocated.next_header = Some(header_for_padding);
            }

            assert!(self.size > size_used + HEADER_SIZE);
            self.size -= size_used;
            self.next_header = Some(header_for_allocated);
            Some(allocated_addr as *mut u8)
        }
    }
}

impl Drop for Header {
    fn drop(&mut self) {
        panic!("Header should not be dropped");
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Header @ {:#018X} {{ size: {:018X}, is_allocated: {} }}",
            self as *const Header as usize,
            self.size,
            self.is_allocated()
        )
    }
}

/// First-Fit 方式のグローバルアロケータ。
/// 空き領域を Header の連結リストとして管理し、先頭から走査して
/// 最初に見つかった「要求を満たせる空き領域」から切り出して返す。
/// `first_header` は連結リストの先頭で、`RefCell` で内部可変性を確保している
/// （`#[global_allocator]` で要求される `&self` 経由のアロケート操作のため）。
pub struct FirstFitAllocator {
    first_header: RefCell<Option<Box<Header>>>,
}

#[global_allocator]
pub static ALLOCATOR: FirstFitAllocator = FirstFitAllocator {
    first_header: RefCell::new(None),
};

unsafe impl Sync for FirstFitAllocator {}

unsafe impl GlobalAlloc for FirstFitAllocator {
    /// `Box::new` 等の組み込みヒープ確保から呼ばれるエントリポイント。
    /// 実体は `alloc_with_options` に委譲する。
    ///
    /// # Safety
    /// `GlobalAlloc` トレイトの契約に従い、`layout` は size > 0 かつ
    /// アライン要件を満たす有効な `Layout` でなければならない。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_with_options(layout)
    }

    /// 確保済み領域を解放する。`ptr` の直前にある Header を Box として復元し、
    /// is_allocated を false に戻したうえで `Box::leak` で所有権を抜く。
    /// leak することで Header の Drop（panic 実装）を回避し、領域は連結リスト上に
    /// 「空き」として残り続ける（ヘッダ自体はメモリ領域の一部なので解放しない）。
    ///
    /// # Safety
    /// `ptr` は本アロケータの `alloc` が返したポインタでなければならない。
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let mut region = Header::from_allocated_region(ptr);
        region.is_allocated = false;
        Box::leak(region);
    }
}

impl FirstFitAllocator {
    /// First-Fit 走査の本体。連結リストを先頭から辿り、最初に
    /// `Header::provide` が `Some` を返した領域のポインタを返す。
    /// どの領域からも切り出せなかった場合は null を返す（Rust の Allocator 規約）。
    pub fn alloc_with_options(&self, layout: Layout) -> *mut u8 {
        let mut header = self.first_header.borrow_mut();
        let mut header = header.deref_mut();

        loop {
            match header {
                Some(e) => match e.provide(layout.size(), layout.align()) {
                    Some(p) => break p,
                    None => {
                        header = e.next_header.borrow_mut();
                        continue;
                    }
                },
                None => {
                    break null_mut::<u8>();
                }
            }
        }
    }

    /// UEFI から取得したメモリマップを走査し、CONVENTIONAL_MEMORY（OS が自由に
    /// 使える通常メモリ）のみを空き領域として連結リストに登録する。
    /// それ以外の領域（ファームウェア予約、MMIO 等）は触らない。
    pub fn init_with_mmap(&self, memory_map: &MemoryMapHolder) {
        for e in memory_map.iter() {
            if e.memory_type() != EfiMemoryType::CONVENTIONAL_MEMORY {
                continue;
            }
            self.add_free_from_descriptor(e);
        }
    }

    /// 1つのメモリディスクリプタを空き領域として連結リストの先頭に挿入する。
    ///
    /// 物理アドレス 0 番地は null ポインタと区別がつかなくなるため、
    /// 先頭ページ（4KiB）を切り捨ててから登録する。残りサイズが
    /// 4KiB 以下になった領域はヘッダを置く余地が無いので捨てる。
    ///
    /// 挿入は「新しい header を先頭に置き、元の先頭を next にぶら下げる」方式。
    /// `RefCell` の borrow を二度に分けているのは、`replace` の borrow を
    /// 一度 drop しないと次の borrow_mut が panic するため。
    fn add_free_from_descriptor(&self, desc: &EfiMemoryDescriptor) {
        let mut start_addr = desc.physical_start() as usize;
        let mut size = desc.number_of_pages() as usize * 4096;

        if start_addr == 0 {
            start_addr += 4096;
            size = size.saturating_sub(4096);
        }

        if size <= 4096 {
            return;
        }

        let mut header = unsafe { Header::new_from_addr(start_addr) };

        header.next_header = None;
        header.is_allocated = false;
        header.size = size;

        let mut first_header = self.first_header.borrow_mut();
        let prev_last = first_header.replace(header);

        drop(first_header);

        let mut header = self.first_header.borrow_mut();

        header.as_mut().unwrap().next_header = prev_last;
    }
}
