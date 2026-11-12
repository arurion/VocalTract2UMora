# VocalTract 移植の進捗メモ (feat/vocal-tract-model)

作業ブランチ: `feat/vocal-tract-model`。再開時はこのメモとコミット履歴を読むこと。

## 基準データの作り方 (VTL をネイティブでビルド)
```
apt-get install -y g++ cmake            # 必要なら
git clone https://github.com/TUD-STKS/VocalTractLabBackend-dev /tmp/vtlsrc
cd /tmp/vtlref && cp <repo>/tools/vtl_reference/dump.cpp .
g++ -O1 -std=c++17 -D_USE_MATH_DEFINES -w -I/tmp/vtlsrc/include dump.cpp /tmp/vtlsrc/src/VocalTractLabBackend/*.cpp -o dump
./dump /tmp/vtlsrc/resources/JD3.speaker > ref.txt      # 全 69 形状の面積関数
```
注意: `VocalTract` / `Tube` は巨大でスタックに載らない。必ず `new` でヒープに確保する。

## 完了
- `geom.rs`   (Geometry: 点・線・円・楕円)
- `splines.rs` (Bezier3D / LineStrip2D / LineStrip3D)
- `surface.rs` (三角形メッシュと垂直面の交線。全三角形を番号順に走査)

## 検証済み (ネイティブ VTL の出力と数値一致)
- 静的な基準表面 13 枚 (PALATE, MANDIBLE, UPPER_TEETH, LOWER_TEETH_ORIGINAL, 軟口蓋 3 枚, 喉頭 4 枚, EPIGLOTTIS_ORIGINAL, UVULA_ORIGINAL): 誤差 < 2e-8
- 動的表面のうち舌に依存しないもの 16 枚 (UPPER/LOWER_COVER, LOWER_TEETH, 唇, RADIATION, 左右の充填面, 鏡像版, UVULA): 誤差 < 2e-7
- 基準ダンプ: `tests/data/vtl_surfaces_a.txt` (形状 a の全 32 表面)、`tests/data/vtl_tubes.txt` (全 69 形状の面積関数)

## 移植時に気づいた VTL の癖 (そのまま保つこと)
- `restrictTongueParams` は TTX/TTY/TCX/TCY 以外の `param[].x` を書き換えたまま戻さない。後続の曲線(根・舌背)は書き換え後の `x` を使う。
- `Point2D::normalize` / `turnLeft` は自身を書き換えて返す。一時値に対しても使われる。
- `LineStrip2D::delPoint` はキャッシュ済みの曲線パラメータを無効化しない。
- `getUniformParam` などは最後に一致した区間を採用する。
- `calcRadiation` では `L, v, h` が前のリブの値を引き継ぐ。
- `VocalTract` / `Tube` はスタックに載らない巨大オブジェクト。

## 舌の移植の依存関係 (読み終わった部分)
`calcTongue` (3801-4296) → `restrictTongueParams` (4384-4675), `limitEllipsePos` (4675-4777), `getEllipseTangent` / `getCircleTangent` (Geometry.cpp 1035, 1146), `tongueSideParamToElevation_cm` (4296), `verifyTongueRibNormal` (5020), `calcTongueRibs` (4777-5020)。
`calcTongueRibs` は `getCrossProfiles` (5879-6680) に依存するので、断面の機構を先に移植する。

## 次
1. 中心線・断面: calcCenterLine (5062-5469) / verifyCenterLineNormal (5469) / calcCrossSections (5549) / getRawCuts (5736) / getCrossProfiles (5879-6680) / insert*ProfileLine (6680-6871) / getCrossSection (6871) / crossSectionsToTubeSections (7307) / getTube (7567)
2. 舌: 上記の依存関係の順に移植 (calcTongue 本体、restrictTongueParams、limitEllipsePos、calcTongueRibs)
3. 全 69 形状の面積関数を `tests/data/vtl_tubes.txt` と比較
4. 各段階を VTL のネイティブ出力と数値比較(中間配列も dump して検証)
