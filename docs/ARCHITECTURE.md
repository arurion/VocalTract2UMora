# VocalTract2UMora アーキテクチャ設計書

調音物理モデリング音声合成で UTAU 原音（WAV + oto.ini）を自動生成する、ブラウザ完結型オーサリングツール。

## 1. 全体構成

```
┌────────────────────────── Browser (GitHub Pages / localhost) ──────────────────────────┐
│  coi-serviceworker  →  COOP/COEP 付与  →  SharedArrayBuffer / Wasm threads 有効化      │
│                                                                                        │
│  Main thread (TypeScript + Vite)             Worker pool (module workers)              │
│  ├ UI: 声質 / 骨格 / 子音 / スクラブ / バッチ   ├ synth worker  : vt2umora-wasm          │
│  ├ Canvas 2D: 矢状断面, グラフ                 ├ batch worker  : リスト一括レンダリング │
│  └ AudioWorklet: リアルタイム試聴              └ zip: 軽量な自前 STORE ZIP ライタ(TS)   │
│        ▲ SharedArrayBuffer (ring buffer / 解析トレース)                                 │
└────────┼───────────────────────────────────────────────────────────────────────────────┘
         ▼
   crates/wasm  (wasm-bindgen 薄いバインディング)
         ▼
   crates/core  (依存ゼロの純Rust: LF声門波 / 声道 / 子音 / oto)
```

原則: **物理計算はすべて Rust core、UI と I/O（JSON, ZIP, ファイル）は TypeScript**。core は外部クレート依存ゼロ。

## 2. ディレクトリ構成

```
.
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ core/                   # 音響・物理（純Rust, std のみ）
│  │  └─ src/{lf,geometry,oto}.rs   (+ 計画: tract, consonant, batch)
│  └─ wasm/                   # wasm-bindgen バインディング
├─ web/
│  ├─ public/coi-serviceworker.js   # サイトルートに配置される
│  ├─ src/main.ts
│  └─ src/wasm-pkg/           # wasm-pack 出力（git管理外）
├─ docs/ARCHITECTURE.md
└─ .github/workflows/deploy.yml
```

## 3. コアモジュール設計

| 機能 | モジュール | 要点 |
|---|---|---|
| ① LF声門波 | `lf` | Rd → (Ra, Rk, Rg) は Fant 2004 回帰（実装済）。dUg/dt を生成し、Spectral Tilt は FFT で算出。Breath(cmH₂O) は声門下圧として声帯自励振動の駆動に使用、Aspiration Noise は声門開口度で変調。 |
| ② 話者ジオメトリ | `geometry` | 声道長・口腔/咽頭比・鼻腔容積スケール。JSON は TS 側で import/export（スキーマに `version` を持たせる）。 |
| ③ 子音エディタ | `consonant`（計画） | 基準形状 /aka/ /iki/ /uku/ の3点を定義。前後母音を母音三角形の重心座標 (u,v) に写し、3形状を重心補間（三角形内の双線形/重心補間）して目標形状を生成。口蓋化音は手動上書きターゲットを優先する。 |
| ④ スクラブ | `tract` + TS描画 | 声道は断面積関数 A(x,t) の管モデル。各フレームで最小断面積・声門開口度・舌口蓋接触フラグをトレースとして出力し、TS が時刻指定で参照（再計算不要でコマ送り可能）。 |
| ⑤ バッチ/oto | `batch`, `oto` | リスト（くろ州式長モーラ / CVVC）を読み込み、1エントリずつ合成。トレースの閉鎖開始・破裂・VOT（声帯振動開始）から oto を決定（下記）。 |

### oto.ini 自動算出規則（初期案）
- 左ブランク = 発音開始（最初の調音動作）の少し手前
- オーバーラップ = 閉鎖開始〜先行母音との重なり位置
- 先行発声 = 破裂/摩擦の開始、または声帯振動開始のうち UTAU の接続点として適切な方（子音種別で分岐）
- 子音部 = 先行発声を超えて母音が安定するまでの固定領域
- 右ブランク = 母音の定常部終了（負値で末尾基準）

## 4. 配布・ランタイム

- **coi-serviceworker**: `web/public/` に置くことで `dist/` ルートへ出力。`index.html` の `<head>` 先頭で読み込む。初回ロード時に自動リロードして COOP/COEP を有効化。localhost では Vite の `server.headers` でも同等のヘッダを付与。
- **互換性ポリシー（重要）**: 対象は「可能な限り全てのブラウザ」。`SharedArrayBuffer` / Wasm マルチスレッドは**任意の高速化**であり、必須にしない。Android WebView 系ブラウザ（Via 等）では service worker が有効でも `crossOriginIsolated` が false のままになることを実機で確認済み。
  - 動作モード: `threaded`（isolated かつ SAB 利用可）/ `basic`（それ以外）。`web/src/capabilities.ts` で起動時に検出し、UI に表示する。
  - `basic` を基準実装とする: 単一スレッド Wasm を Web Worker で実行、バッチは「Worker 複数 × 1エントリ/ジョブ」で並列化（SAB 不要）。
  - AudioWorklet への供給: `threaded` では共有リングバッファ、`basic` では `postMessage` 転送（Transferable）。AudioWorklet 非対応環境では、生成済みバッファを `AudioBufferSourceNode` で再生。
  - Wasm スレッド（nightly + atomics + build-std）は `threaded` 限定の最適化として後続PRで評価。ビルドは別バイナリにせず、まず basic のみで機能を完成させる。
- **AudioWorklet**: 上記のとおり。

## 5. ビルド / デプロイ

- `npm run wasm` : `wasm-pack build crates/wasm --target web --out-dir web/src/wasm-pkg`
- `npm run build` : wasm → `tsc --noEmit` → `vite build`
- Pages は `/<repo>/` 配下のため CI で `VITE_BASE=/<repo>/` を渡す。
- `.github/workflows/deploy.yml`: PR ではテスト+ビルドのみ、main への push で Pages にデプロイ。**リポジトリ設定で Pages の Source を「GitHub Actions」にする必要あり。**

## 6. ロードマップ（PR 単位）

1. `chore/architecture`（本PR）: 雛形・CI
2. `feat/lf-source`: LF波形生成 + dUg/dt・Spectral Tilt プレビュー（声質エディタ）
3. `feat/geometry-editor`: 骨格エディタ + JSON I/O
4. `feat/tract-sim`: 声道管モデル、声帯自励振動、流体（気流）計算
5. `feat/consonant-editor`: 3点補間と口蓋化オーバーライド
6. `feat/scrub-preview`: 矢状断面 + 3種グラフ
7. `feat/batch-oto-zip`: バッチレンダラー、oto 自動生成、ZIP 出力

## 7. ライセンスと由来

- 本プロジェクトは **GPL-3.0-or-later**。VTL（GPLv3）の移植を許容し、CHIMERA（MIT）は著作権表示を保持して取り込む。
- 由来の記録は `THIRD_PARTY_NOTICES.md` に集約する。流用したモジュールは次の表に追記すること。

| モジュール | 由来 |
|---|---|
| `lf` | 自前実装（LF モデルは標準的な定義。VTL/CHIMERA は検証用に参照） |
| `geometry`, `oto` | 自前実装 |
| `tract`, `consonant`, `batch`（計画） | 未定（VTL / CHIMERA 流用の可能性あり） |

## 8. 運用ルール

- `main` へ直接 push しない。`chore/` `feat/` ブランチ + PR。
- 依存は最小限（現状: web は `vite`, `typescript` のみ／core は依存ゼロ）。
