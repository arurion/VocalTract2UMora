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
| ① LF声門波 | `lf`, `fft` | **実装済み**。時間は T0=1、振幅は Ee=1 に正規化。Rd → (Ra, Rk, Rg) は Fant 2004 回帰。ε は戻り相の連続条件、α は正味流量ゼロ条件を数値で解く（解なしの組み合わせは `NoSolution` を返し、UI が通知）。dUg/dt・Ug・スペクトル（FFT）・傾き（調波レベルの log2 f 回帰, 5 kHz まで）・H1−H2 を `preview` が返す。Breath は流量ゲイン `(Ps/8)^1.5`（しきい値 2.5 cmH₂O 未満で声帯振動なし）、Aspiration は Ug で変調した白色ノイズ。これらはプレビュー用の簡易モデルで、声道結合後の気流計算では `tract` 側の物理に置き換える。 |
| ② 話者ジオメトリ | `geometry` | **実装済み**。声道長 8〜22 cm・口腔の割合 0.25〜0.75・鼻腔容積 0.3〜2.5 倍（範囲は core の定数と `web/src/geometry.ts` で一致させる）。core は範囲外を `clamped()` で補正し、一様管近似の共鳴 F_n=(2n−1)c/4L を参考値として返す。JSON は TS 側で入出力し、`schema: "vt2umora.speaker-geometry"` と `version: 1` を必須とし、範囲外・型違いは読み込みを拒否してエラー表示する。 |
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

完了: 雛形・CI、ライセンス(GPL-3.0)、互換性ポリシー、LF 声質エディタ、骨格エディタ、VTL 調査。
決定事項と根拠は `docs/VTL_NOTES.md` を参照（VTL 型の調音パラメータモデルまで移植する）。

1. `feat/tract-sim`: Tube（区間列）+ TDS ソルバー、LF 流量源で駆動、一様管での検証、Wasm 速度計測
2. `feat/vocal-tract-model`: `.speaker` 読み込み、19 個の調音パラメータ → 矢状断面の輪郭 → 面積関数、舌口蓋の接触判定
3. `feat/consonant-editor`: 基準形状（19 次元）の 3 点補間と口蓋化の手動上書き
4. `feat/scrub-preview`: スクラブ、最小断面積・声門開口度・接触ハイライト
5. `feat/batch-oto-zip`: バッチレンダラー、oto 自動算出、ZIP 出力

## 7. ライセンスと由来

- 本プロジェクトは **GPL-3.0-or-later**。VTL（GPLv3）の移植を許容し、CHIMERA（MIT）は著作権表示を保持して取り込む。
- 由来の記録は `THIRD_PARTY_NOTICES.md` に集約する。流用したモジュールは次の表に追記すること。

| モジュール | 由来 |
|---|---|
| `lf`, `fft` | 自前実装（LF は標準的な定義。CHIMERA の声門は面積波形＋気道側で流量計算する方式で LF ではないため流用なし） |
| `geometry`, `oto` | 自前実装 |
| `tract`, `consonant`, `batch`（計画） | 未定（VTL / CHIMERA 流用の可能性あり） |

## 8. 運用ルール

- 基本は `chore/` `feat/` ブランチ + PR。小さく安全な変更に限り、オーナー許可のもとで `main` へ直接 push も可（CI が必ず走るので、壊した場合は revert する）。
- 依存は最小限（現状: web は `vite`, `typescript` のみ／core は依存ゼロ）。
