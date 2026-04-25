# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

RustでUEFIベースのOSを開発するプロジェクトです。x86_64アーキテクチャ上で動作するベアメタルUEFIアプリケーションをビルドします。メインのOSコードは`wasabi/`ディレクトリにあります。

## 開発環境

一貫した開発環境のためにDockerを使用しています：

- **開発コンテナの起動**: `docker-compose up -d`
- **コンテナに入る**: `docker exec -it rust_ubuntu bash`
- **作業ディレクトリ**: コンテナ内の`/wasabi`で作業を行います

Dockerイメージ（Ubuntu 22.04）には以下が含まれます：
- Rustツールチェイン（rustup経由でインストール）
- x86_64エミュレーション用のQEMU
- ビルドツールと依存関係

## ビルド方法

プロジェクトは`x86_64-unknown-uefi`をターゲットとし、特定のnightly Rustツールチェインが必要です。

**ビルドコマンド**（コンテナ内、またはRust nightly-2024-01-01環境で実行）：
```bash
cd wasabi
cargo build --target x86_64-unknown-uefi
```

**重要なビルド情報**：
- ツールチェイン: nightly-2024-01-01（`wasabi/rust-toolchain.toml`で指定）
- ターゲット: x86_64-unknown-uefi
- 必要なコンポーネント: rustfmt, rust-src
- ビルドすると`.efi`実行ファイルが生成されます

## 実行方法

UEFIアプリケーションはOVMFファームウェアを使ってQEMU上で実行します：

1. ビルドされた`.efi`ファイルを`wasabi/mnt/EFI/BOOT/BOOTX64.EFI`にコピーします
2. OVMFファームウェアは`wasabi/3rd_party/ovmf/RELEASEX64_OVMF.fd`にあります
3. `mnt/`ディレクトリがEFIシステムパーティションとして機能します

**QEMUコマンド例**：
```bash
qemu-system-x86_64 \
  -bios wasabi/3rd_party/ovmf/RELEASEX64_OVMF.fd \
  -drive format=raw,file=fat:rw:wasabi/mnt
```

## アーキテクチャ

**コア構造**：
- `wasabi/src/main.rs` - メインエントリポイント、現在は最小限の実装（`#![no_std]`ベアメタルコード）
- `wasabi/mnt/` - EFIシステムパーティション（gitignore対象）
  - `EFI/BOOT/BOOTX64.EFI` - UEFIブートローダーの配置場所
  - `NvVars` - UEFI不揮発性変数
- `wasabi/3rd_party/ovmf/` - QEMU用のOVMF UEFIファームウェア

**重要な制約**：
- 標準ライブラリなし（`#![no_std]`） - ベアメタルコードです
- UEFIターゲット環境 - 限定的なランタイム環境
- すべての出力はUEFIプロトコル経由で行われ、標準I/Oは使用できません

## Gitワークフロー

- メインブランチ: `main`
- 現在の開発ブランチ: `chapter/002`
- コミットメッセージは日本語

## 重要な注意事項

- `mnt/`ディレクトリはビルド成果物を含むためgitignoreされています
- 異なるホストシステム間で一貫したツールチェインを確保するためにDockerを使用しています
- UEFIアプリケーションには特定の要件があります（no_std、特定のエントリポイント、UEFIプロトコル）
- 現在のコードにある`println!`マクロは、おそらくUEFI固有の実装から来ています
