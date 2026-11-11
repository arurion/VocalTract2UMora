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

## 次
1. VocalTract の静的表面: initSurfaceGrids / initReferenceSurfaces / initLarynx / initJaws / initVelum
2. calcSurfaces / calcLips / calcRadiation
3. 舌: restrictTongueParams / calcTongue / calcTongueRibs
4. 中心線・断面: calcCenterLine / calcCrossSections / getRawCuts / getCrossProfiles / crossSectionsToTubeSections / getTube
5. 各段階を VTL のネイティブ出力と数値比較(中間配列も dump して検証)
