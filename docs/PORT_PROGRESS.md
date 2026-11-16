# VocalTract 移植の進捗メモ

## 状態: 調音パラメータモデル (口腔系) の移植が完了

19 個の調音パラメータ → 3D 表面 → 中心線 → 断面 → 面積関数(40 区間) → `Tube` が、VTL のネイティブ出力と数値一致している。

| 検証 | 結果 |
|---|---|
| 静的な基準表面 13 枚 | 誤差 < 2e-8 |
| 動的表面 32 枚すべて(舌・喉頭蓋を含む、形状 /a/) | 誤差 < 5e-7 |
| 全 69 形状の面積関数(長さ・断面積・調音器官・切歯位置・舌尖側面) | 最悪 5e-10 |
| 合成した母音の共鳴 | /a/ 829・1260 Hz、/i/ 258・2283 Hz、/u/ 312・797 Hz(F2: i > a > u) |

## 基準データの作り方 (VTL をネイティブでビルド)
```
apt-get install -y g++ cmake            # 必要なら
git clone https://github.com/TUD-STKS/VocalTractLabBackend-dev /tmp/vtlsrc
mkdir -p /tmp/vtlref && cd /tmp/vtlref
cp <repo>/tools/vtl_reference/dump.cpp <repo>/tools/vtl_reference/dump_surf.cpp .
SRC=/tmp/vtlsrc/src/VocalTractLabBackend; INC=/tmp/vtlsrc/include
g++ -O1 -std=c++17 -D_USE_MATH_DEFINES -w -I$INC dump.cpp      $SRC/*.cpp -o dump
g++ -O1 -std=c++17 -D_USE_MATH_DEFINES -w -I$INC dump_surf.cpp $SRC/*.cpp -o dump_surf
./dump      /tmp/vtlsrc/resources/JD3.speaker    > vtl_tubes.txt            # 全 69 形状の面積関数
./dump_surf /tmp/vtlsrc/resources/JD3.speaker a  > vtl_surfaces_a.txt       # 形状 a の全 32 表面
```
結果は `crates/core/tests/data/` に置いてある。注意: `VocalTract` / `Tube` は巨大でスタックに載らない。必ず `new` でヒープに確保する。

## 移植したモジュール
`geom.rs` (点・線・円・楕円・接線)、`splines.rs` (Bezier3D / LineStrip2D / LineStrip3D)、`surface.rs` (三角形メッシュと垂直面の交線)、`vocal_tract.rs` (静的表面、`calc_surfaces`、唇、放射面、舌、中心線、断面、管区間、`get_tube`)。

## 移植時に気づいた VTL の癖 (そのまま保つこと)
- `restrictTongueParams` は TTX/TTY/TCX/TCY 以外の `param[].x` を書き換えたまま戻さない。後続の曲線(根・舌背)は書き換え後の `x` を使う。
- `Point2D::normalize` / `turnLeft` は自身を書き換えて返す。一時値に対しても使われる。
- `LineStrip2D::delPoint` はキャッシュ済みの曲線パラメータを無効化しない。
- `LineStrip2D` の交点関数は、失敗時に `t = 1e6`、交点 `(0, 0)` を返す。
- `getUniformParam` などは最後に一致した区間を採用する。
- `calcRadiation` では `L, v, h` が前のリブの値を引き継ぐ。
- 完全閉鎖の形状では、`Tube` が面積に下限 1e-4 cm² を課す(`get_tube` で適用)。
- 交点の探索は、VTL の既定ではタイル分割(`makeFasterIntersections`)だが、全三角形を番号順に調べても同じ結果になる。

## 子音の補間 (consonant.rs)
母音座標・文脈依存形状 56 通りがネイティブ出力と一致 (`tests/data/vtl_consonant_ctx.txt`、ダンプは `tools/vtl_reference/dump_ctx.cpp`)。

## 未移植
鼻腔(19 区間とその結合)、副鼻腔、梨状窩、`GesturalScore`(ジェスチャー列から時系列パラメータ)。

## 次の作業
1. 鼻腔(鼻音 /m n ŋ/ とんに必要)
2. (済) 3 点コーナー母音補間と手動上書き
3. 矢状断面の描画とスクラブ(中間配列 `upper_outline` / `lower_outline` / `tongue_outline` / `center_line` / `tube_section` は取得可能)
4. Wasm への公開(`VocalTract` の API)
