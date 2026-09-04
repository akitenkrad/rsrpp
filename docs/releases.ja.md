[English](releases.md) | **日本語**

# リリース履歴

<details open>
<summary>1.0.25</summary>

- LLM による処理が既定で有効になった（`ParserConfig::new()` が `use_llm: true` を設定する）
  - `OPENAI_API_KEY` が設定されていない場合，実行時に自動的に LLM は無効化される
  - 明示的に無効にするには `--no-llm`（CLI）または `config.use_llm = false`（ライブラリ）を使う
- LLM による節の検証が，LLM の見ていないページの節まで捨てていた問題を修正
  - `merge_sections()` がページ範囲を考慮するようになった: LLM が調べたページ範囲内の節だけを検証する
  - LLM のページ範囲外の節は，フォントにもとづく検出の結果がそのまま保持される

</details>

<details>
<summary>1.0.24</summary>

- Nature 形式や非標準の論文で本文が失われる問題を修正:
  - "Abstract" 見出しを持たない論文（Nature 形式など）向けに，アンカー語の照合による節検出のフォールバックを追加
  - 算出したテキスト領域が小さすぎるときに全ブロックが除外されるのを防ぐため，テキスト領域の退化検出を追加
  - グラフの罫線や図の枠線による誤検出を弾くため，表領域の検出をページ面積の 50% で頭打ちにした
  - 既知の節タイトルを表領域フィルタの対象外にした
- 数式抽出の精度を改善:
  - LLM が抽出した数式テキストが捨てられていた重大なバグを修正．trigram によるブロック対応付けを追加
  - 誤検出を削減: 日付（`2019/2020`），統計（`n = 50 participants`），節の参照
  - 複数文字の数学関数（`sin`, `cos`, `log`），ASCII の指数・添字（`x^2`, `x_i`），文字どうしの分数（`a/b`），ノルムの記法（`||w||`）の検出を追加
  - 数式の出力を `<math>` タグ内の LaTeX 形式に統一（Unicode 記号を LaTeX コマンドへ変換）
  - 構造のみのパターン一致に対して，文脈にもとづく検証を追加
  - 網羅的な退行テスト群を含む 25 個のテストを追加

</details>

<details>
<summary>1.0.23</summary>

- crate のドキュメントとバージョンを更新．

</details>

<details>
<summary>1.0.22</summary>

- テキストのクリーニングと数式マークアップのサポートを追加:
  - キャプション検出（Figure, Table, Algorithm など）のための `cleaner` モジュールを新設
  - 数式の検出と `<math>...</math>` マークアップのための `llm` モジュールを新設
  - `Section` に `math_contents` と `captions` フィールドを追加
  - 数式マークアップ付きの出力を得る `Section::from_pages_with_math()` メソッドを新設
- 新しい CLI オプション:
  - `--include-captions`: キャプションを本文フィールドに含める
  - `--no-math-markup`: 数式の検出とタグ付けを無効にする
  - `--no-llm`: LLM による処理を無効にする
- LLM のモデルを指定する環境変数 `OPENAI_API_MODEL` を新設（既定: gpt-5.2）

</details>

<details>
<summary>1.0.21</summary>

- panic を引き起こす unwrap() の呼び出しを，適切なエラー処理へ修正．

</details>

<details>
<summary>1.0.20</summary>

- macOS における Poppler 25.12.0 との互換性を修正．

</details>

<details>
<summary>1.0.19</summary>

- `fix_suffix_hyphens` をリファクタリングし，31 種類の複合語サフィックスに対応:
  - `-based`, `-driven`, `-oriented`, `-aware`, `-agnostic`, `-independent`, `-dependent`, `-first`, `-native`, `-centric`, `-intensive`, `-bound`, `-safe`, `-free`, `-proof`, `-efficient`, `-optimized`, `-enabled`, `-powered`, `-ready`, `-capable`, `-compatible`, `-compliant`, `-level`, `-scale`, `-wide`, `-specific`, `-friendly`, `-facing`, `-like`, `-style`
- サフィックスのハイフン処理に対するユニットテストを追加．

</details>

<details>
<summary>1.0.18</summary>

- PDF から節タイトルを抽出する方法を更新．

</details>

<details>
<summary>1.0.17</summary>

- `rsrpp.parser` を再構成．
- PDF から節タイトルを抽出する方法を更新．
- テストを更新．

</details>

<details>
<summary>1.0.16</summary>

- `rsrpp` から `init_logger` を削除．

</details>

<details>
<summary>1.0.15</summary>

- 誤字を修正．
- `tracing` ロガーを導入．

</details>

<details>
<summary>1.0.14</summary>

- `rsrpp-cli` が参照する `rsrpp` のバージョンを更新．

</details>

<details>
<summary>1.0.13</summary>

- 依存関係を更新．
- crate のインストール時に sudo を要求するため build.sh を削除．

</details>

<details>
<summary>1.0.12</summary>

- バグ修正: 不要な `println!` を削除．

</details>

<details>
<summary>1.0.11</summary>

- xml のループがファイル末尾で終了するようにバグを修正．

</details>

<details>
<summary>1.0.10</summary>

- verbose モードを追加．
- ページ番号を抽出する処理のバグを修正．

</details>

<details>
<summary>1.0.9</summary>

- 更新: 不正な URL を扱うためのエラーを新たに実装．

</details>

<details>
<summary>1.0.8</summary>

- 更新: PDF ファイル保存時の最大リトライ時間を延長．

</details>

<details>
<summary>1.0.7</summary>

- バグ修正: PDF へ変換した後，処理が完了するまで待つようにした．

</details>

<details>
<summary>1.0.4</summary>

- `get_pdf_info` のバグを修正．
- 細かな改善を実施．

</details>

<details>
<summary>1.0.3</summary>

- cli を追加 -> [rsrpp-cli](https://crates.io/crates/rsrpp-cli)．

</details>

<details>
<summary>1.0.2</summary>

- `Section` モジュールを更新．`content: String` を `content: Vec<TextBlock>` に置き換えた．

</details>
