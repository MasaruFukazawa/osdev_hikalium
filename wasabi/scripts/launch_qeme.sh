#!/bin/bash -e

PROJ_ROOT="$(dirname $(dirname ${BASH_SOURCE:-$0}))"
cd "${PROJ_ROOT}"

PATH_TO_EFI="target/x86_64-unknown-uefi/debug/wasabi.efi"

rm -rf mnt
mkdir -p mnt/EFI/BOOT
cp ${PATH_TO_EFI} mnt/EFI/BOOT/BOOTX64.EFI

qemu-system-x86_64 \
    -m 4G \
    -bios 3rd_party/ovmf/RELEASEX64_OVMF.fd \
    -drive format=raw,file=fat:rw:mnt \
    -boot menu=off
