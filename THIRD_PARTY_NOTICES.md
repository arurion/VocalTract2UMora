# Third-party notices

このプロジェクトは **GPL-3.0-or-later** で配布されます（`LICENSE` 参照）。
以下の外部プロジェクトのコード・アルゴリズムを参照／流用する場合は、流用したファイル先頭にも出典とライセンスをコメントで明記し、下表を更新してください。

| 出典 | ライセンス | 入手元 | 本リポジトリでの利用範囲 |
|---|---|---|---|
| VocalTractLab (VTL) | GPL-3.0 | https://www.vocaltractlab.de/ | 調査済み（`docs/VTL_NOTES.md`）。`crates/core/src/tds.rs` / `tube.rs` は VTL の `TdsModel` / `Tube`（Peter Birkholz, GPL-3.0）の定式化・初期値に基づく Rust 移植（口腔系のみ。ファイル先頭に出典を明記）。LF の基準数値(ε, α)は `lf.rs` のテストで照合に使用（数値のみ）。声道形状（`VocalTract`）・声帯モデルの移植は未着手 |
| CHIMERA (chimera-vocal-synth) | MIT, Copyright (c) 2026 Jameson Hodge | https://github.com/jamesondh/chimera-vocal-synth | 調査済み・流用なし（現時点）。声門 `sources/glottis.rs` は LF ではなくパラメトリックな声門面積波形で、流量は気道ネットワーク（ベルヌーイ＋慣性）側で計算する設計。声道・音素計画・ジェスチャ（`airway/`, `phonetics/`, `control/`）は今後の流用候補。 |
| coi-serviceworker v0.1.7 | MIT | https://github.com/gzuidhof/coi-serviceworker | `web/public/coi-serviceworker.js`（無改変で同梱） |

## MIT ライセンスの流用物について
CHIMERA は Rust 実装のため、移植なしで取り込める部分が多い見込み。MIT のコードを取り込む場合、その著作権表示と許諾文を保持する必要があります。流用時は、該当ファイル内、または本ファイルに原文を追記してください。

## データファイルについて
VTL に同梱される話者データ等のデータファイルは、コードと別に利用条件を確認してから取り込むこと。
