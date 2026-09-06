[English](releases.md) | **日本語**

# リリース履歴

<details open>
<summary>2.0.1</summary>

**poppler の出力上限は 2 点だけを見て置かれており，その間を取りこぼしていた．**
`DEFAULT_POPPLER_MAX_FILES_PER_PAGE` は 200 で，健全な文書（4.0 files/ページ）と，
当時知られていた唯一の暴走文書（63,500/ページ．1pt のタイリングパターンで塗られた図）の
間に置かれていた．その後 2,906 本のコーパスで測ると，間に人の住む帯があった —
16 本が 231〜3,491 files/ページを書く．いずれも 1 秒以内に終わり 12〜154MB を残す．
図版が多いだけで暴走ではないのに，200 はその全部を弾いていた（コーパスの 0.6%）．

- `DEFAULT_POPPLER_MAX_FILES_PER_PAGE` 200 -> 8,000．測った帯に対して 2.3 倍の余裕がある．
  暴走文書は 136,000 ファイル（書きたがる 1,079,890 の 8 分の 1）で止まる．失うのは
  «最初の 1 秒で止まる» という性質で，そこは bytes 側が受け持つ．
- `DEFAULT_POPPLER_MAX_BYTES_PER_PAGE` 10 MiB -> 20 MiB．帯の 1 本が 11 ページで 154MB を
  書くため，files だけ上げても片方の上限からもう片方へ移るだけだった．357 ページの
  参照スキャンには依然 43 倍の余裕がある．
- `MIN_POPPLER_FILE_BUDGET` は既定では効かなくなる（1 ページで 4 倍を買える）．per-page を
  下げる呼び出し側のために残す．

API の変更は無い．これらのフィールドを明示的に設定していた呼び出し側に影響は無い．

</details>

<details>
<summary>2.0.0</summary>

**破壊的変更が 2 つある．** どちらもシグネチャではなく挙動の変更である．

- **一時ファイルが config の寿命に紐づくようになった．** パースは専用のディレクトリで作業し，
  `ParserConfig` への最後のハンドルが消えた時点でそれを削除する — 正常終了でも，途中の
  return でも，panic の巻き戻し中でも同じである．従来は何も削除されていなかった:
  `clean_files()` はあったがテストからしか呼ばれておらず，仮に呼んでも `pdftohtml -c` は
  図のパスを集めた後も画像を書き続けるので，取りこぼしは残ったはずである．
  **`config.pdf_figures` から取り出したパスは config より長生きしない**．config が生きて
  いるうちにファイルの方をコピーすること．
- **CLI が panic で終了しなくなった．** 従来はすべての失敗が `unwrap` だったので，壊れた
  PDF も書き込めない出力先も poppler が終わらない文書も，等しく終了コード 101 と
  バックトレースになっていた．今は終了コードが種類を伝える: `0` 書き出し成功，`1` 入力または
  環境，`2` コマンドライン，`3` poppler が制限に達した．101 を見ていたスクリプトは修正が要る．

**poppler に制限を設けた．** 一部の PDF は poppler に際限なく書かせる — 1pt と 4pt の
タイリングパターンで塗られた図は `pdftohtml` にタイル 1 枚ごとの PNG を吐かせ，実測では
1 回のパースで 1,079,890 ファイル・7.4 GB に達して戻ってこなかった．poppler の各呼び出しは
監視され，制限時間か出力量のどちらかを超えると停止される．両方が要るのは，1,000 ページの
スキャンは正当な理由で遅いのであって，決して終わらない文書とは区別しなければならないからである．
上限はページ数に比例し，短い文書には下限がある: `DEFAULT_POPPLER_TIMEOUT`（900 秒），
`DEFAULT_POPPLER_MAX_FILES_PER_PAGE`（8,000），`DEFAULT_POPPLER_MAX_BYTES_PER_PAGE`
（20 MiB）．いずれも `ParserConfig` から変更できる．超過は
`converter::PopplerLimitError` として返るので，メッセージの照合ではなく型で判別できる．

**放置された作業ディレクトリを回収するようになった．** `SIGKILL` はデストラクタを走らせない
ので，強制終了された実行はディレクトリを残し，poppler が親より長く生き残って書き続けることも
ある．各プロセスは最初の `ParserConfig::new()` で 1 度だけ掃除を行い，所有プロセスが消えたと
確実に言えるディレクトリだけを対象にするので，並行して走るパースが互いのファイルを奪うことはない．

**節の検出を 2,812 本で計測した．** 直前の数日で入った変更 — 見出し候補をその位置で判定する
配置パス，キャプションと表領域の規則，ページ単位の段判定 — を，標本ではなくコーパス全体に
対して検証した．**本文は失われていない**: `--keep-dropped` の出力は poppler が読んだものと
1 バイトも違わず，コーパス全体では差引 +4,830 字である．ALL CAPS の親見出しと違うインデントに
座る小見出しは親の節に畳まれる．本文は保たれ，構造は粗くなる．

**ドキュメント．** README は表紙になり，詳細は英語と日本語で `docs/` に移った．そこにあった
ライブラリの例は `rsrpp::parser::structs` から import していたが，これは実在しない．

</details>

<details>
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
