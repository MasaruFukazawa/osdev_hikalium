#!/bin/bash

PROJ_ROOT="$(dirname $(dirname ${BASH_SOURCE:-$0}))"
cd "${PROJ_ROOT}"

# cargo test のランナーから呼ばれた場合は $1 にテストバイナリのパスが入る。
# 通常実行 (make launch) ではビルド済みの wasabi.efi をブートする。
PATH_TO_EFI="${1:-target/x86_64-unknown-uefi/debug/wasabi.efi}"

rm -rf mnt
mkdir -p mnt/EFI/BOOT
cp "${PATH_TO_EFI}" mnt/EFI/BOOT/BOOTX64.EFI

qemu-system-x86_64 \
    -m 4G \
    -bios 3rd_party/ovmf/RELEASEX64_OVMF.fd \
    -drive format=raw,file=fat:rw:mnt \
    -device isa-debug-exit,iobase=0x4,iosize=0x04 \
    -serial stdio \
    -display none \
    -boot menu=off
RETCODE=$?

# isa-debug-exit に Success(0x1) を書くと QEMU は (0x1<<1)|1 = 3 で終了する。
# qemu.rs の QemuExitCode::Success に対応。
if [ $RETCODE -eq 3 ]; then
    printf "\nPASS\n"
    exit 0
elif [ $RETCODE -eq 0 ]; then
    exit 0
else
    printf "\nFAIL: QEMU returned $RETCODE\n"
    exit 1
fi
