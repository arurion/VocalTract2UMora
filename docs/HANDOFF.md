# 引き継ぎ資料 (次のセッションの Claude へ)

最終更新: 2026-11-21。**最初にこのファイルを読み、次に `docs/PORT_PROGRESS.md` と `docs/VTL_NOTES.md`(§9 以降が最新) を読むこと。**
リポジトリ: `arurion/VocalTract2UMora` (GPL-3.0-or-later)。公開ページ: https://arurion.github.io/VocalTract2UMora/

## 1. いまどこまで出来ているか

目標は「調音物理モデリング音声合成で UTAU 音源 (WAV + oto.ini) を自動生成するブラウザツール」。
**コア(Rust)は、ほぼ一式が動く(鼻腔も実装済み)。足りないのは (b) Wasm への公開と UI、(c) 品質の詰め、(d) 「ん」の扱い。**

| 要件 | 状態 |
|---|---|
| ① LF 声門波エディタ | **完成** (core `lf.rs` + web `voice-view.ts`)。公開ページで動作 |
| ② 話者ジオメトリ(骨格) | UI と JSON 入出力は完成 (web `geometry-view.ts`)。**ただし VocalTract(調音モデル)にはまだ繋がっていない** (§4-d) |
| ③ 3点コーナー母音補間 | **core 完成** (`consonant.rs`、VTL とネイティブ比較で一致)。**UI なし** |
| ④ スクラブ・プレビュー | **データは core で取れる** (`render.rs` の `TraceSample` / `GeometryFrame`)。**UI なし** |
| ⑤ バッチ + oto.ini + ZIP | **core 完成** (`batch.rs` ほか、Python の zipfile/wave/cp932 で検証済み)。**UI・Wasm 公開なし**。鼻音(な・ま・ガ行鼻濁音)は対応済み。**「ん」(撥音)は `split_morae` が `Dangling` で拒否する(未設計)** |

テスト: `cargo test -p vt2umora-core` → **98 件パス、約 10 秒**。CI(`.github/workflows/deploy.yml`)は PR でテスト+ビルド、main への push で GitHub Pages にデプロイ。

## 2. 作業のしかた (オーナーの許可事項)

- **オーナーは、ブランチ作成・push・PR 作成・CI 確認・マージ・マージ済みブランチ削除を、私の判断で進めてよいと言っている。main への直接 push も許可済み**(小さく安全な変更に限る)。致命的な削除以外は自由に整理してよい。「ある程度完成するまで報告なしでもよい」とも言われている。
- 基本の流れ: `feat/...` ブランチ → push → PR → CI 完了を待つ(API で `actions/runs?branch=...` を確認)→ マージ(`merge_method: merge`)→ ブランチ削除。**CI の結果は必ず自分で見て報告する**(過去に「base が main でないPRをマージして内容が main に入らなかった」事故があった。PR の base は main にすること)。
- GitHub の PAT: オーナーがチャットで渡す。コマンド内の環境変数 `GH_TOKEN` でだけ使い、**ファイル・git config・ログに残さない**。clone/push は `https://x-access-token:${GH_TOKEN}@github.com/...`、API は `Authorization: Bearer`。このファイルにトークンは書いていない。**チャットに平文で出た PAT は 2 本ある(古い方と新しい方)。オーナーに revoke を勧めること**(新しい方は Administration など権限が広い)。
- 返信は**日本語**。簡潔に。見つけた不具合や、自分のミスは隠さず報告する(例: 前に書いた Rg の式のバグ、VTL の弱い雑音源が起動しない癖)。
- **UI は `/mnt/skills/plugins/tinted-glass-ui-philosophy/SKILL.md` に従う**(ライト基調、色付き半透明ガラスは枠のみ、Bento Grid 禁止、画面全体を 1 つのアプリウィンドウとして構成、モバイルは右からのドロワー、など)。UI を作る前に必ず読み直すこと。既存の `web/src/style.css` と `voice-view.ts` / `geometry-view.ts` が実例。
- **「可能な限り全てのブラウザで動く」が要件**。オーナーの Via ブラウザ(Android WebView)では `crossOriginIsolated=false` で `SharedArrayBuffer` が使えないことを実機で確認済み。`web/src/capabilities.ts` が `threaded` / `basic` を判定する。**`basic`(単一スレッド Wasm + Web Worker + postMessage)を基準にする**。並列化は Worker を複数立てて 1 録音ずつ処理させる方式(SAB 不要)。

## 3. コードの地図 (`crates/core/src/`)

音の流れ: `speaker`(話者ファイル)→ `vocal_tract`(19 個の調音パラメータ → 面積関数)→ `tube` → `tds`(時間領域の音響シミュレーション)+ `glottis`(声帯)+ `synth`(合成ループ)。時間方向は `gesture`(スコア → パラメータ曲線)と `render`(スコアを音とトレースに)。

| ファイル | 役割 |
|---|---|
| `lf.rs`, `fft.rs` | LF 声門波、声質プレビュー(UI あり) |
| `geometry.rs` | 話者スカラー(声道長・口腔比・鼻腔容積)。調音モデルとは未接続 |
| `xml.rs`, `speaker.rs` | 最小 XML パーサーと `.speaker` 読み込み(解剖データ、19 パラメータ、69 形状、声帯形状) |
| `geom.rs`, `splines.rs`, `surface.rs` | VTL の Geometry / Splines / Surface の移植 |
| `vocal_tract.rs` | **VTL の VocalTract の移植**(約 1,900 行)。全 69 形状の面積関数がネイティブ VTL と最悪 5e-10 で一致 |
| `consonant.rs` | 母音三角形への写像、3 形状の補間、**(子音, 母音) ごとの手動上書き** (`ConsonantEditor`) |
| `gesture.rs` | ジェスチャスコア → 5 次臨界減衰のパラメータ曲線(VTL と最悪 5e-10 で一致) |
| `tube.rs`, `tds.rs` | 管モデルと TDS ソルバー(**口腔系のみ**、雑音源あり) |
| `glottis.rs`, `iir.rs`, `synth.rs` | 声帯(GeometricGlottis)、チェビシェフ、合成ループ |
| `render.rs` | スコア → 音 + トレース(最小断面積・声門・肺圧・舌口蓋の接触)+ 矢状断面フレーム |
| `mora.rs` | 日本語子音の表、モーラ/モーラ列のスコア作成(タイミングは実験で調整) |
| `oto.rs` | 事象検出(音の開始・解放・声帯振動開始・母音開始)から oto の値を算出 |
| `kana.rs` | かな → モーラ、ローマ字ファイル名、Shift_JIS(Python cp932 と全かなで一致) |
| `batch.rs`, `zip.rs` | 録音リスト解析、1 行ずつ合成、oto.ini、ZIP(無圧縮の自前実装) |

`crates/wasm/src/lib.rs` の公開 API は**まだ LF プレビューと `geometry_info` だけ**。`web/src/` は声質・骨格の 2 画面(サイドバーの「子音」「スクラブ」「書き出し」は「準備中」の表示)。

録音リストの書式(`batch.rs` 先頭のコメント): 1 行 = 1 回の録音(一息)。ひらがな/カタカナ、空白可、`#` はコメント。1 モーラだけの行なら普通の 1 音 1 ファイル、長い行なら連続録音(モーラごとに oto 1 行)。**「くろ州式長モーラリスト」の正確な書式はオーナーに確認していない**(推測で汎用の形式にした)。

## 4. 次にやること (優先順)

### a. 鼻腔の移植 (**完了**。PR #16)
- **形状**: `tube.rs` の `init_nasal_cavity` / `set_velum_opening`(19 区間 × 0.6 cm、軟口蓋の開きから最初の 4 区間を 2 次補間)。`Tube.nose` は `sections` とは別の `Vec`。`vtl_tubes.txt` の `N` 行と全 69 形状で 1e-5 以内で一致(`area_functions_of_all_shapes_match_the_original`)。`interpolate_supraglottal` は軟口蓋開口を線形補間する(VTL と同じ)。
- **TDS**: `tds.rs` は 84 区間(口腔系 65 + 鼻腔 19)+ 放射電流 4 本 = 88 電流。`LAST_PHARYNX`(40) の出力が [口の先頭 41, 鼻の先頭 65] の分岐。分岐行の式は `TdsModel.cpp` 2137–2169 行と照合済み(対角 −E_B−E_A−F、分岐先 −E_A−G、右辺 L_AB·uB + L_A·uD の項)。放射は口と鼻孔の 2 組で、出力は両者の和(`nostril_flow()` で鼻孔のみ取得可)。乱流損失は「前区間の出力が 1 本だけ」の条件付きで鼻側にも適用。**VTL と同様に唇ダイポール雑音は鼻孔の放射行にも入る**(VTL の癖をそのまま移植)。
- **消去順序**: コレスキーは**最小フィルイン**順(同点は次数→番号)。単純な最小次数法では分岐で非単体頂点を選んでフィルインが 41 個出たため。テスト `elimination_order_adds_no_fill_in` が 0 を保証する。
- **検証**: 口を閉じて軟口蓋を 1 cm² 開くと鼻孔のみから出力、最初の共鳴は約 280 Hz(鼻音の第 1 共鳴)。軟口蓋が閉じていれば鼻孔流量はほぼ 0。`batch.rs` の「な・ま」が実際にレンダリングでき、閉鎖区間の鼻音 RMS が母音の約 70%。
- **未実装(VTL の既定で有効)**: 副鼻腔・梨状窩・皮膚放射・経軟口蓋結合。副鼻腔・梨状窩は鼻音の反共振(アンチフォルマント)に効くので、音質を詰めるなら次。
- **「ん」**: `split_morae` が単独の「ん」を拒否する。UTAU 上の位置づけ(直前母音の鼻音化、閉鎖の持続、oto の先行発声)を決めてから実装する。

### b. Wasm API と UI
`crates/wasm` に公開する: 話者読み込み(`JD3.speaker` を `include_str!` で同梱するか、ユーザーのファイル)、`ConsonantEditor`(基準形状の取得・上書きの設定/削除)、モーラ/スコアのレンダリング(音+トレース+フレーム)、`batch::render_line`、`oto_ini_bytes`、`voicebank_zip`。型付き配列で返すこと。
画面(サイドバーの「準備中」を埋める):
1. **子音エディタ**(③): 子音を選び、/a/ /i/ /u/ の 3 基準形状を表示・編集、後続母音に応じた補間結果のプレビュー、口蓋化(き・け・拗音)の**手動上書きの保存/解除**、JSON の入出力。
2. **スクラブ・プレビュー**(④): 矢状断面(`GeometryFrame`: `tongue` 正中線、`upper`、`lower`、`contact`)のコマ送り、最小断面積グラフ・声門開口度(`glottal_area_cm2` / `glottal_amplitude_cm`)グラフ・舌口蓋接触のハイライト。
3. **書き出し**(⑤): リスト入力 → Worker プールで並列生成(進捗表示)→ ZIP ダウンロード。
UI は §2 の方針に従うこと。

### c. 骨格エディタを調音モデルに繋ぐ
今の骨格エディタ(声道長・口腔比・鼻腔容積)は独立したスカラーで、`VocalTract` の解剖データ(`speaker.anatomy`)に反映されていない。声道長のスケーリングを解剖データ(口蓋・顎・咽頭・喉頭の寸法)に適用する仕組みが必要。

### d. 品質の詰め(耳で確認が必要。コードだけでは決まらない)
- 生成した WAV を UTAU / OpenUtau で実際に鳴らし、oto の値(`oto.rs` の規則: 左ブランク = 音の開始 − 20 ms、先行発声 = 母音開始まで、オーバーラップ = 先行発声の 1/3、子音部 = 先行発声 + 30 ms、右ブランクは母音開始から 250 ms を残す)と、母音の鳴り(ドイツ語話者のデータなので日本語の「う」「え」「お」に違和感が出るかも)を調整する。
- モーラのタイミング(`mora.rs`)は /ka/ で調整しただけ。各子音の VOT・閉鎖長・摩擦の長さは未検証。波形の先頭にスタートアップのクリックが出る(リードの無音を長めにする等)。
- 未対応: 促音(っ)、ヴ、ふぁ行など、CVVC の VC エイリアス(`a k` 形式)、15 モーラ母音優先リスト特有の書式。
- 速度: 声道計算 2.2 ms/フレーム(400 Hz)+ 合成。リリースビルドで実時間の約 0.5 倍。Wasm / Android 実機は**未計測**。長いリストは時間がかかる。最適化の余地: `get_cross_profiles` のメモリ確保、断面のフレーム間再利用、フレームレートを下げるオプション。

## 5. 検証の仕組み (VTL をネイティブでビルドして基準データを作る)

移植の正しさは、VTL(`TUD-STKS/VocalTractLabBackend-dev`、GPL-3.0)をこの環境で `g++` ビルドして、中間値と最終値を書き出して比較して保証している。**環境(`/tmp`)はセッションごとに消える**ので、必要なら作り直す。

```
apt-get update && apt-get install -y g++ cmake gdb          # 必要なら
git clone https://github.com/TUD-STKS/VocalTractLabBackend-dev /tmp/vtlsrc   # (PAT が要るかもしれない)
mkdir -p /tmp/vtlref && cd /tmp/vtlref && cp <repo>/tools/vtl_reference/*.cpp .
SRC=/tmp/vtlsrc/src/VocalTractLabBackend; INC=/tmp/vtlsrc/include
g++ -O1 -std=c++17 -D_USE_MATH_DEFINES -w -I$INC dump.cpp $SRC/*.cpp -o dump
./dump /tmp/vtlsrc/resources/JD3.speaker > vtl_tubes.txt      # 全 69 形状の面積関数と鼻腔
```
`dump_surf.cpp`(全 32 表面)、`dump_ctx.cpp`(母音座標と子音形状)、`dump_score.cpp`(スコアの曲線)も同様。**`VocalTract` / `Tube` は巨大でスタックに載らない(`new` でヒープに確保すること)**。基準データは `crates/core/tests/data/` にコミット済み。

VTL のソースを読むときの補助スクリプト(コメント行と空行を除いて行範囲を表示):
```
python3 - <<'PY' > /tmp/show.py
import re,sys
f=sys.argv[1]; a=int(sys.argv[2]); b=int(sys.argv[3])
t=open(f,'rb').read().decode('latin-1').split('\n')   # ドイツ語コメントは UTF-8 ではない
print('\n'.join(s for s in (re.sub(r'\s*//.*$','',l).strip() for l in t[a-1:b]) if s))
PY
```

## 6. VTL との意図的な差分・未実装

- 副鼻腔・梨状窩・皮膚放射・経軟口蓋結合: 未実装(鼻腔は実装済み)。
- 弱い雑音源が起動しない VTL の癖(現在振幅を 0 に戻す処理)は直した(息漏れ声のため)。`tds.rs` のコメントと `VTL_NOTES.md` §6。
- パラメータ曲線はスコアの終わり + 10 ms まで(VTL は残りを中立値で埋める)。
- 子音形状は `ConsonantEditor` から取るので手動上書きが効く。「ふ」用の `ll-labial-fricative` は VTL の形状ではなく、両唇閉鎖の LD に +0.14 cm したもの(`consonant.rs`)。
- 交点の探索は、VTL の既定(タイル分割)ではなく全三角形(+ 同じ側の打ち切り)。結果は同じ。

## 7. はまりやすい点

- 出力を `| head -N` で切ると、Rust の `println!` が panic して(`panic = "abort"`)「Aborted」になる。バグではない。
- `cargo test` のフィルタは 1 つしか渡せない。`[profile.test] opt-level = 2` を設定済み(音響計算が遅いため)。
- ローカルの rustc は 1.75 で新しい wasm-bindgen がビルドできない。wasm クレートの型チェックは、一時的に `wasm-bindgen = "=0.2.92"` に固定して `cargo check -p vt2umora-wasm` し、**終わったら必ず元に戻し、`Cargo.lock` を消す**(ロックファイルはコミットしていない)。本番ビルドは CI(stable)で確認される。`wasm-pack` はローカルに無い。
- `web/` の型チェックは、生成物 `web/src/wasm-pkg/` が無いと通らない(CI は wasm-pack を先に実行する)。ローカルでは仮の `.d.ts` を置いて `npx tsc --noEmit`、その後削除。
- ブラウザがこの環境に無いので、**UI の見た目と操作は実機(オーナーの Via)でしか確認できない**。UI の PR では「未確認」と明記し、デプロイ後にオーナーに見てもらう。

## 8. オーナーとの約束・メモ

- 参照元: VocalTractLab(GPL-3.0、流用可)、CHIMERA (`jamesondh/chimera-vocal-synth`、MIT、流用可。ただし声門は LF ではなく面積波形方式なので LF では使っていない。声道・音素計画・ジェスチャ側は今後の候補)。出典は `THIRD_PARTY_NOTICES.md` に記録する。
- `resources/JD3.speaker`(VTL リポジトリ同梱のデータ)は GPL-3.0 として扱って同梱している。作者側から別の条件が示されたら同梱をやめる。
- 配布は GitHub Pages(`coi-serviceworker` は導入済みだが、WebView 系では効かない前提で設計)。
