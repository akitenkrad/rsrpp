[English](library.md) | **日本語**

# ライブラリ

```rust
use rsrpp::{config::ParserConfig, models::Section, parser::parse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ParserConfig::new(); // LLM は既定で有効
    let verbose = true;

    // URL でもローカルファイルのパスでもよい
    let url = "https://arxiv.org/pdf/1706.03762";

    // PDF をパースしてページ構造を得る
    let pages = parse(url, &mut config, verbose).await?;

    // 節の構造へ変換する（基本形）
    let sections = Section::from_pages(&pages);

    // 数式マークアップ付き（数式は <math>...</math> タグで囲まれる）
    let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);

    // JSON 形式で出力する
    let json = serde_json::to_string_pretty(&sections_with_math)?;
    println!("{}", json);

    Ok(())
}
```

## 一時ファイル

パースはシステムの一時ディレクトリの下に専用のディレクトリを作ってそこで作業し，
**生成したものは config への最後のハンドルが消えた時点ですべて削除される** — 正常に終わった
場合も，途中で return した場合も，panic で巻き戻っている最中も同じである．`clean_files()`
は今もあり，同じディレクトリを削除する．呼ぶかどうかは任意になり，2 度呼んでもエラーには
ならない．

知っておく価値のある帰結が 2 つある．

- **config から取り出したパスは config より長生きしない．** `config.pdf_figures` から
  パスをコピーしておき，config が drop された後にそのファイルを読もうとしても，ファイルは
  もう存在しない．config が生きているうちにファイルの方をコピーしておくこと．
- **殺されたプロセスは自分の後片付けができない．** `SIGKILL` はデストラクタを走らせないので，
  強制終了された実行はディレクトリを残すし，poppler が親より長く生き残ってそこへ書き続ける
  こともある．同じ一時ディレクトリで次に走ったパースが，過去の実行が放置したものを回収する —
  対象は所有プロセスが消えたと確実に言えるディレクトリだけなので，並行して走っているパースが
  互いのファイルを奪うことはない．自分で発火させたければ `sweep_abandoned_temp_dirs()` が
  公開されている．

## poppler の出力制限

PDF の中には，poppler に際限なく書き出させるものがある．実際に見つかった 17 ページの論文は，
幅 1 ポイントと 4 ポイントのタイリングパターンで図が描かれているために `pdftohtml` がタイル
ごとに PNG を出力する．1 回のパースで作られたファイルは 100 万個を超え，容量は 7.4 GB に
達し，しかも終わる気配が無かった．放っておけば，この 1 本がコーパス全体の実行を止める．

そこで poppler の呼び出しはすべて監視され，2 つの制限のいずれかを超えたら kill される．
時間制限だけでは足りない．1,000 ページのスキャンされた予稿集は真っ当な理由で遅いのであって，
「遅い」と「無界に出力する」は区別しなければならない．そのため，文書のページ数あたりで出力量
にも上限を置く:

| フィールド | 既定値 | 定数 |
|---|---|---|
| `poppler_timeout` | 900s | `DEFAULT_POPPLER_TIMEOUT` |
| `poppler_max_files_per_page` | 8,000 | `DEFAULT_POPPLER_MAX_FILES_PER_PAGE` |
| `poppler_max_bytes_per_page` | 20 MiB | `DEFAULT_POPPLER_MAX_BYTES_PER_PAGE` |

短い文書には比例配分ではなく下限を与える．2 ページの論文が 400 ファイルに縛られないように
するためで，`MIN_POPPLER_FILE_BUDGET`（2,000 ファイル）と `MIN_POPPLER_BYTE_BUDGET`
（200 MiB）がそれにあたる．

既定値は健全な文書が必要とする量に対して十分な余裕がある — 357 ページのスキャンでも 1 ページ
あたり約 4 ファイル・470 KB である — ので，パースできない論文への答えが「制限を上げること」で
あることはまずない．信頼できない PDF を扱っていて，もっと早く失敗させたい場合は下げるとよい:

```rust
use std::time::Duration;

let mut config = ParserConfig::new();
config.poppler_timeout = Duration::from_secs(120);
config.poppler_max_files_per_page = 50;
```

## エラーハンドリング

制限への抵触は型の付いたエラーとして報告されるので，呼び出し側はメッセージの文字列を照合せずに
それと判別できる:

```rust
use rsrpp::converter::{PopplerLimit, PopplerLimitError};

match parse(url, &mut config, false).await {
    Ok(pages) => { /* ... */ }
    Err(e) => {
        match e.downcast_ref::<PopplerLimitError>().map(|e| e.limit()) {
            Some(PopplerLimit::Time) => eprintln!("this document is too slow to finish"),
            Some(PopplerLimit::Files) | Some(PopplerLimit::Bytes) => {
                eprintln!("this document makes poppler write without bound")
            }
            None => eprintln!("parsing error: {}", e),
        }
    }
}
```

`PopplerLimit` は `#[non_exhaustive]` である．4 つ目の制限が追加されても，上の `match` が
壊れることはない．

## 何が捨てられたか

黙って捨てられるものは何も無い．取り除かれた断片はすべて `ParserConfig::dropped_texts` に，
どのページから来たのかと，なぜ捨てられたのかとともに入っている:

```rust
let pages = parse(url, &mut config, false).await?;

println!("coverage: {:?}", config.coverage());
for dropped in &config.dropped_texts {
    println!("p{} [{}] {}", dropped.page, dropped.reason, dropped.text);
}
```

カバレッジの数値が何を数えているかは
[テキストカバレッジ](cli.ja.md#テキストカバレッジ) を参照．

## 設定のカスタマイズ

```rust
use rsrpp::config::ParserConfig;

let mut config = ParserConfig::new();
config.use_llm = false;             // LLM の経路を完全に飛ばす
config.extract_references = true;   // 構造化された参考文献（OPENAI_API_KEY が必要）

let pages = parse("path/to/paper.pdf", &mut config, true).await?;
```

`ParserConfig` は `#[non_exhaustive]` である．構造体リテラルではなく `new()` で作り，
必要なフィールドだけを書き換えること．
