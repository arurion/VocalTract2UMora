# VocalTractLab (VTL) 調査メモ

対象: https://github.com/TUD-STKS/VocalTractLabBackend-dev （C++、GPL-3.0、本プロジェクトと互換）。
**読んだ範囲**: `LfPulse.cpp`、`Glottis.h`、`GeometricGlottis.h`、`Tube.h`、`TdsModel.h`、`Synthesizer.h`、`Speaker.h`（いずれも全体または公開インターフェース）。
**未読**: `VocalTract.cpp`(250 KB)、`TdsModel.cpp`(83 KB)、`GesturalScore.cpp`(130 KB)、`TwoMassModel.cpp`、`TriangularGlottis.cpp`、`AnatomyParams.cpp`、`Surface/Geometry/Splines`。移植に着手する前に該当部分を読むこと。

## 1. 合成パイプライン

```
GesturalScore ─▶ VocalTract パラメータ ─▶ Tube(断面列) ┐
                                          Glottis パラメータ ┤─▶ Synthesizer.add() ─▶ TdsModel ─▶ 音声
                                                             └ 約2.5 ms(110 サンプル@44.1 kHz)ごとに、管形状を線形補間
```

- **Tube**: 1 cm 刻みの短い管の列。気管 23 + 声門 2 + 咽頭 16 + 口腔 24 + 鼻腔 19 + 副鼻腔 4 + 梨状窩 5。各区間は面積・長さ・体積・壁の質量/抵抗/剛性・「どの調音器官の領域か」(声帯/舌/下切歯/下唇/他)を持つ。調音器官の情報は摩擦音の雑音源の生成に使われる。
- **TdsModel**: 分岐管を電気回路(L, C, R)に見立てて時間領域で解く。枝電流を毎サンプル、SOR(ガウス・ザイデル)かコレスキー分解で解く。乱流損失、軟らかい壁、雑音源(単極子/双極子)、皮膚放射、梨状窩、経軟口蓋結合などをオプションで切り替える。全て CGS 単位。
- **Glottis(基底クラス)**: 制御パラメータの先頭 2 つは常に `FREQUENCY` と `PRESSURE`。`incTime(dt, 4点の圧力)` で時間発展し、`getTubeData` で声門の管(長さ・断面積)を返す。
  - `GeometricGlottis`(既定): F0, 肺圧, 下端/上端 X, チンク面積, 位相遅れ, 相対振幅, 二重脈動, パルス歪度, フラッター, 気息強度。静的パラメータは厚さ・長さ・安静時 F0・チンク長。
  - ほかに `TwoMassModel`、`TriangularGlottis`。
  - 音源フィルタ型の励振として `LfPulse` / `ImpulseExcitation` / `VowelLf` もある。
- **LfPulse**: OQ/SQ/TL(Te = OQ, Tp = Te·SQ/(1+SQ), Ta = TL)で指定し、ε は戻り相の連続条件(ニュートン法)、α は正味流量ゼロ条件(はさみ打ち)で求める。本プロジェクトの `lf.rs` と同じ数式構造。

## 2. 検証

`lf.rs` の `agrees_with_vocaltractlab_reference_values` は、VTL の LfPulse の式から求めた (ε, α) の数値を基準にして、3 組の独立なパラメータで私たちのソルバーと一致(ε 相対 1e-3、α 絶対 2e-3)を確認する。**数値のみを使用し、VTL のコードは含めていない。**

## 3. 本プロジェクトへの示唆

1. **音源は 2 系統を持つ**:
   - (A) LF 流量源を声門位置に入れる(`TdsModel.flowSourceAmp` に相当)。決定的でパラメータが少なく、声質エディタ(①)がそのまま使える。oto.ini の時刻算出は、まずこちらで確実に行う。
   - (B) 自励振動の声門(`GeometricGlottis` 相当)。肺圧 Breath で駆動する物理系。
2. **tract-sim の最初の一歩**: 区間(Tube)モデルと TDS ソルバーの最小サブセットを Rust で実装し、LF 流量源で駆動する。検証として「一様管 17.5 cm のスペクトルピークが一様管近似の共鳴(500/1500/2500 Hz、骨格エディタの表示値)の近傍に出る」ことを確認する(唇の放射補正で少し低くなる想定)。
3. **子音プレビューの要件**(舌と口蓋の接触判定、最小断面積)には、面積関数だけでなく矢状断面の輪郭が必要。VTL は調音パラメータ(顎・舌・唇・軟口蓋など約 20 個)から輪郭と面積関数を作る。**面積関数レベルのターゲットだけで進めるか、調音パラメータモデルまで移植するかは、設計上の大きな分岐**であり、`feat/consonant-editor` の前に決める。
4. **話者データ**: VTL は `.speaker` ファイル(解剖データ)を使う。コードとは別に、データの再利用条件を確認してから取り込むこと。
5. **計算量**: VTL は 44.1 kHz のサンプル毎に枝電流の連立方程式を解く。Wasm 単一スレッドで、Android 端末でも非リアルタイムのバッチ生成が現実的な速度で回るかを、tract-sim の早い段階で計測する。
