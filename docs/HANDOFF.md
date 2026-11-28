# 引き継ぎ資料 (次のセッションの Claude へ)

最終更新: 2026-11-21。**最初にこのファイルを読み、次に `docs/PORT_PROGRESS.md` と `docs/VTL_NOTES.md`(§9 以降が最新) を読むこと。**
リポジトリ: `arurion/VocalTract2UMora` (GPL-3.0-or-later)。公開ページ: https://arurion.github.io/VocalTract2UMora/

## 1. いまどこまで出来ているか

目標は「調音物理モデリング音声合成で UTAU 音源 (WAV + oto.ini) を自動生成するブラウザツール」。
**コア(Rust)は、ほぼ一式が動く(鼻腔も実装済み)。足りないのは (b) Wasm への公開と UI、(c) 品質の詰め、(d) 「ん」の VC 別名など細部。**

| 要件 | 状態 |
|---|---|
| ① LF 声門波エディタ | **完成** (core `lf.rs` + web `voice-view.ts`)。公開ページで動作 |
| ② 話者ジオメトリ(骨格) | UI と JSON 入出力は完成 (web `geometry-view.ts`)。**ただし VocalTract(調音モデル)にはまだ繋がっていない** (§4-d) |
| ③ 3点コーナー母音補間 | **core 完成** (`consonant.rs`、VTL とネイティブ比較で一致)。**UI なし** |
| ④ スクラブ・プレビュー | **データは core で取れる** (`render.rs` の `TraceSample` / `GeometryFrame`)。**UI なし** |
| ⑤ バッチ + oto.ini + ZIP | **core 完成** (`batch.rs` ほか、Python の zipfile/wave/cp932 で検証済み)。**UI・Wasm 公開なし**。鼻音(な・ま・ガ行鼻濁音)は対応済み。「ん」(撥音)も対応済み(後述) |

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
- **「ん」(撥音) — 実装済み**(UTAU 制作界隈の慣習を調査して設計。出典: まいこ氏のブログ「『n あ』なんて存在しない件(ンについて)」と「まいこさん流CVVCの原音設定」)。
  - 日本語の「ん」は後続音で n / m / ŋ / N(鼻母音)に変わる。UTAU では n・m・ŋ は同じ「ん」として扱い、N(「ン」)だけが別物。母音の前の「ん」は N になり、「n あ」という発音は存在しない。収録は「全部ん」「全部ン」「両方」の三択で、両方録るのが無難とされる。
  - 実装: `split_morae` は「ん」を `consonant: "N"`、母音は直前モーラを継承(`ー` と同じ)で返す。先頭の「ん」は `Dangling`。`mora::moraic_nasal_symbol(次の子音)` が調音位置の同化を決める(m・p・b の前→m、k・g の前→ng、n・t・d・ch・ts・j・z・r の前→n、母音・s・sh・h・f・y・w・語末→N)。`MoraItem.moraic` が真のとき、鼻音を 1 モーラ分(`MoraOptions::moraic_nasal_s`、既定 0.20 s)保持し、母音は発音しない(直前母音が文脈)。N は閉鎖なしで軟口蓋だけを開く。
  - oto: 閉鎖型は alias「ん」、N 型は alias「ン」で別エントリ。先行発声は**閉鎖の開始**(VC と同じ「母音が終わる位置」)、オーバーラップは先行発声の 1/3(慣例)、右ブランクは鼻音の終わりで切る(次のモーラを巻き込まない)。`oto::detect_moraic_nasal` / `compute_oto_steady`。ファイル名のローマ字は N(例 `kaNka`)。
  - 検証: 閉鎖型は母音の約 70%、N は母音と同程度の音量(`moraic_nasals_assimilate_and_get_their_own_segments`)。
  - **未実装**: 連続音・CVVC 用の `a ん` のような VC 別名(現状は CV 的な「ん」「ン」のみ)。ラ行の前の「ん」は n 扱い(界隈でも発音が人により異なるとされる)。鼻音化した母音(あ→ア̃)はまだ作らない(N は直前母音の形のまま軟口蓋を開く)。

### a2. くろ州式リストの調査と対応(新)
出典: くろ州氏の note「ボカロ(広義)曲に私が登場しててうれしい」「『くろ州式長モーラリスト』セット」ほか(txt の直接ダウンロードは robots で自動取得不可。内容は記事の説明から確認した)。
- **リストの構造**: 行は `_` で始まり、`_` は休符(1307 モーラ版は行中にも出る。語頭音と語中音を区別するため)。8 モーラ版は「1 セクション = 1 子音」で、母音パターン `[0,0,N,1,0,2,0,1]` を 1 つずつずらして 5 行で 1 セクション(理論値)。N は「ん」。子音は `?kstnhmyrwgzdbpKSCTNHFMRGJBPV`(大文字は拗音・ふぁ行・ヴ行などの別系列)。語尾に子音単体を足す記法(`…かんk`)もある。16・44・1018・1307 モーラ版は「人間には発音できない」ほど長く、物理モデルなら問題なく読める(が速度に注意、下記)。
- **対応済み**(`kana.rs` / `mora.rs`): 休符 `_`(行頭は無視、行中は `MoraOptions::rest_s`=0.30 s の無音で肺気圧オフ、oto エントリなし)、小書き母音(てぃ とぅ でぃ どぅ つぁ〜つぉ ふぁ〜ふぉ すぃ ずぃ うぃ うぇ うぉ いぇ きぇ しぇ ちぇ ひぇ …)、ヴ行(v、Shift_JIS の ヴ で出力)。くろ州式 8 モーラ構造の行が休符・ん・特殊かな入りで通る(`kurosu_style_lines_with_rests_and_special_kana_render`)。
- **未対応**: 語尾の子音単体(`k` など ASCII)、大文字の特殊子音記号、くろ州式の `_` の語頭/語中の区別を oto の alias に反映すること。
- **最大の未解決(設計)**: 現在の oto は alias が裸のかな(「か」)の **CV 単独音**用。連続音リストを読んで連続音(VCV)音源を作るなら、alias を `a か`(前の母音+かな)・`- か`(語頭)・`a R`(語尾)・`a ん` にし、前の母音の定常部から切る必要がある(先行発声は子音の始まり、オーバーラップは先行発声の 1/3 が慣例)。物理モデルなら直前の母音を指定して直接合成できるので、リストの文脈どおりに録る必要すらない。ここをどうするか(リストの形式で自動判別するか、出力形式をユーザーが選ぶか)を決める。
- **速度(対策済み・計測済み)**: `cargo run --release -p vt2umora-core --example bench` で内訳が出る。計測結果(リリースビルド): 声道形状 `calculate_all` が 1.7 ms/ステップ × 400 ステップ/秒 = 音声 1 秒あたり 0.67 秒(うち `calc_cross_sections` が 75%。`surface.rs` の交差判定はすでに矩形カット・リブ対カリング済みで、これ以上は設計変更が要る)、音響 `set_tube + step` が 0.25 秒。対策: (1) パラメータの変化が 1e-6 未満なら前の管を再利用(`render.rs` `REUSE_TOLERANCE`)、(2) `RenderOptions::geometry_stride`(形状を 400 Hz の曲線の何ステップごとに計算するか、間は管を線形補間。既定 1 = 原版どおり、`BatchOptions` の既定は 2。ストライド 2 で oto は最大 2 ms、4 で 6 ms しかずれない=テスト `a_coarser_geometry_step_keeps_the_oto_and_the_level`)。結果: 8 モーラ行 10 本のビルドが 32 秒 → 16 秒。内蔵リスト全 145 行で単一スレッド約 4 分(ネイティブ)。**次の手は Wasm 側の Web Worker 並列(行ごとに独立)**と、必要なら音響側(`prepare` の毎サンプル係数計算)の最適化。

### a3. 出力形式(単独音・連続音・CVVC)と内蔵リスト (**実装済み**)
- `batch::OtoStyle`: `Single`(CV 単独音。alias は裸のかな)、`Vcv`(連続音。`a か` / `ん か` / 語頭・休符後は `- か`)、`Cvvc`(裸の CV + VC `a k` `a ng` `i sh` `a n` …)。前の文脈は `batch::context_of`。
- oto の計算(`oto.rs`): `OtoEvents.vowel_end_s`(前の音が終わり子音が始まる点 = 閉鎖の開始、閉鎖しない子音は調音運動の開始)。`compute_oto_vcv`(前の母音の中 110 ms から始め、先行発声は CV と同じく子音の終わり、オーバーラップは先行発声の 1/3 で上限 40 ms)、`compute_oto_vc`(前の母音から子音の閉鎖の終わりまで。破裂のバーストと後続母音は含めない)。ん/ン は VCV で `あ ん`/`あ ン`、CVVC で `a n`/`a m`/`a ng`/`a N`。
- 同じ alias は最初の 1 件を残す(`dedupe_entries`)。使われない WAV は ZIP から外す(`assemble_voicebank`)。レンダリング自体は別の場所(Worker 並列など)でやってよい: `render_line(index, ..)` の index は results 内の位置。
- `lists::kurosu8_list()`: くろ州式 8 モーラ構造(29 区画 × 5 行 = 145 行)を記事のルールで生成する内蔵リスト。母音パターン `[0,0,N,1,0,2,0,1]` を 5 通りずらすと、(前の母音 × 次の母音) の 25 通り、語頭、ん が全部出る(テスト `built_in_list_covers_every_context`)。物理モデルは録音不要なので、これが「全音素」の代わりになる。
- CLI: `cargo run --release -p vt2umora-core --example build_bank -- <single|vcv|cvvc> <list.txt|builtin> <out.zip> [最大行数]`。
- **未実装**: 語尾の `a R`、CVVC の `- か` 系、リストの形式からの自動判別(今は呼び出し側が `OtoStyle` を選ぶ)。

### b. Wasm API と UI
**書き出し(⑤)は実装・実機検証済み。子音エディタ(③)とスクラブ(④)が残り。**

- **Wasm API**(`crates/wasm/src/lib.rs`): `Engine`(話者。空文字で同梱の JD3 を使う。`render_line(行, index, BatchConfig) -> RenderedLine`)、`BatchConfig`(形式 `single|vcv|cvvc`、f0、圧力、母音の長さ、ん・休符の長さ、サンプリング周波数、形状ストライド、ノイズ種)、`plan_list(text) -> ListPlan`(各行の読み取り結果とエラー)、`builtin_list()`、`new_assembler() -> Assembler`(`add_wav` / `add_entry` / `add_error` / `finish` / `zip` / `oto_text` / `errors`)。ワーカーが `RenderedLine` から wav と oto 値を取り出して `postMessage` で返し、メインスレッドの `Assembler` が重複排除と ZIP 化をする。SharedArrayBuffer は不要(各ワーカーが独立した Wasm インスタンス)で、Android の WebView 系ブラウザでも動く設計。
- **Web**: `web/src/batch-worker.ts`(ワーカー)、`batch-runner.ts`(ワーカープール。動的割り当て、中止、進捗)、`batch-types.ts`、`export-view.ts`(画面)。画面: 形式(単独音/連続音/CVVC)、声(f0・母音の長さ)、速さ(標準/高精度/高速=形状ストライド 2/1/4、同時に生成する行数)、音源名、リスト(内蔵リスト/ファイル読み込み/直接編集、読めない行の指摘)、進捗(何行目・残り時間)、oto.ini プレビュー、ZIP ダウンロード。
- **実機検証**(headless Firefox 161、Selenium): 内蔵リスト全 145 行を連続音形式で生成 → 7 分 38 秒(この環境は 2 コア・2 ワーカー)、oto 948 件、WAV 145 本、ZIP 46 MB、oto.ini は Shift_JIS。ワーカー 1 本あたりの速さはネイティブの約 3 倍遅い(Firefox の Wasm、この環境の CPU)。コア数が多い端末では並列数を上げれば比例して速くなる。
- **見つけて直した既存バグ**: `voice-view.ts` が `defaults()`(Wasm 呼び出し)をモジュール先頭で実行しており、`init()` 前に例外が出てアプリ全体が起動しなかった(CI はビルドしか見ていなかった)。`createVoiceView` の中で初期化するよう修正。**実ブラウザでの起動確認を CI に入れる価値がある**(Playwright が使える環境なら、`vite preview` を立てて `#export` を開く程度のスモークテスト)。
- **ブラウザ検証の手順**(サンドボックス向け): apt には Rust の wasm32 が無く、static.rust-lang.org も遮断される。conda-forge(conda.anaconda.org は許可)から micromamba を取り、`micromamba create -p /opt/rs -c conda-forge rust rust-std-wasm32-unknown-unknown wasm-pack binaryen` と `... -p /opt/ff firefox geckodriver`、`pip install selenium`。`npm run build` の後に `web/dist` を COOP/COEP ヘッダー付きの静的サーバーで配信し、Selenium で操作する。
- **未実装(次)**: 話者ファイルの読み込み(`Engine::new` は対応済み。UI の入口がない)、骨格エディタの値をエンジンに反映(§c)、`Assembler` のメモリ(WAV を全部保持。1307 モーラのような長い行では要注意)、リスト形式からの自動判別。

画面(サイドバーの「準備中」を埋める):
1. **子音エディタ**(③): 子音を選び、/a/ /i/ /u/ の 3 基準形状を表示・編集、後続母音に応じた補間結果のプレビュー、口蓋化(き・け・拗音)の**手動上書きの保存/解除**、JSON の入出力。Wasm には `ConsonantEditor` の取得・上書きの API がまだ無い。
2. **スクラブ・プレビュー**(④): 矢状断面(`GeometryFrame`: `tongue` 正中線、`upper`、`lower`、`contact`)のコマ送り、最小断面積グラフ・声門開口度(`glottal_area_cm2` / `glottal_amplitude_cm`)グラフ・舌口蓋接触のハイライト。
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
