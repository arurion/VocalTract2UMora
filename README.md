# VocalTract2UMora

> **作業を引き継ぐ人(Claude を含む)は、まず [docs/HANDOFF.md](docs/HANDOFF.md) を読んでください。**

調音物理モデリング音声合成による UTAU 音源（WAV + oto.ini）自動生成ツール。設計は [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) を参照。

## 開発

```sh
# 必要: Rust (stable) + wasm32-unknown-unknown target, wasm-pack, Node 20+
cd web
npm install
npm run dev
```

## ライセンス

GPL-3.0-or-later。外部コードの出典は [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) を参照。
