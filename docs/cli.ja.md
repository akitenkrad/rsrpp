[English](cli.md) | **日本語**

# CLI

```bash
# arXiv の論文をパースする
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention_paper.json --verbose

# ローカルのファイルをパースする
rsrpp --pdf ./paper.pdf --out output.json

# 数式のマークアップを無効にする（数式検出を行わない）
rsrpp --pdf ./paper.pdf --out output.json --no-math-markup

# キャプションを別フィールドにせず本文に含める
rsrpp --pdf ./paper.pdf --out output.json --include-captions

# LLM を使った処理を無効にする
rsrpp --pdf ./paper.pdf --out output.json --no-llm
```

## オプション

| オプション | 説明 |
|--------|------|
| `--pdf <URL\|PATH>` | 入力 PDF（URL またはローカルファイルのパス） |
| `--out <PATH>` | 出力 JSON のパス（既定: output.json） |
| `--verbose` | 詳細な出力を有効にする |
| `--no-llm` | LLM を使った処理を無効にする |
| `--include-captions` | キャプションを本文フィールドに含める |
| `--no-math-markup` | 数式の検出とマークアップを無効にする |
| `--extract-references` | 構造化された参考文献を抽出する（`OPENAI_API_KEY` が必要） |
| `--keep-dropped` | フィルタが捨てたものをすべて収めた `Unassigned` 節を末尾に追加する |

## 環境変数

| 変数 | 説明 | 既定値 |
|------|------|--------|
| `OPENAI_API_KEY` | OpenAI の API キー（LLM 機能に必要） | - |
| `OPENAI_API_MODEL` | LLM 処理に使うモデル | `gpt-5.2` |

## 終了コード

コーパス単位の実行では，*この論文は飛ばして先に進め* と *止まれ，何かがおかしい* を区別
できなければならない．終了コードがそれを伝える:

| コード | 意味 |
|------|------|
| `0` | JSON を書き出した |
| `1` | 入力または環境の側が失敗した: PDF が存在しないか読めない，出力パスに書き込めない，出力名が `.json` でない，poppler が入っていない |
| `2` | コマンドラインが誤っている（未知のフラグ，`--pdf` の欠落） |
| `3` | poppler がいずれかの制限に達して打ち切られた — この文書は終わらないので，バッチは次へ進むべきである |

panic で終了することはなく，バックトレースも出さない．失敗は標準エラー出力に 1 行だけ書かれる:

```
rsrpp: pdftohtml exceeded the output limit of 3400 files and was killed (poppler_max_files_per_page); ...
```

バッチスクリプトが「この論文は諦める」と扱うべきなのはコード `3` である．これは
[poppler の制限](library.ja.md#poppler-の出力制限) から来ていて，メッセージにはどの制限に
達したかが書かれている．それ以外はいずれも注意を払う価値がある．`1` はたいてい，スクリプトが
前提としていた環境になっていないことを意味する．

## テキストカバレッジ

一部のテキストは意図的に取り除かれる．検出された表領域の中のセルと，本文としては細すぎ短すぎる
ブロック（図の軸ラベルや凡例）である．「意図的に取り除いた」が「黙って失った」に変わらないよう，
どのパースも何を保持したかを報告する:

```
WARN rsrpp::parser: Text coverage 93.2% (32018 of 34364 source chars kept, 32018 chars
in blocks). Discarded — narrow block: 196 fragments / 1128 chars, outside text area: 2
fragments / 62 chars, table region: 145 fragments / 1156 chars.
```

この行はカバレッジが 99% を下回ると `WARN`，上回れば `INFO` で記録されるので，ひどく壊れた
パースは出力を diff しなくても見つかる．カバレッジが数えるのは英数字だけなので，空白や読み順の
違いが損失として計上されることはない．

`--keep-dropped` を付けると，捨てられた断片を収めた `Unassigned` 節が末尾に追加され，
poppler が読み取った内容に対して出力が無損失になる:

```bash
rsrpp --pdf paper.pdf --out output.json --keep-dropped --include-captions
```

論文がテキストを失っているように見えたら，まずこれを使うとよい．たいていの場合テキストはまだ
そこにある — 節の中ではなく，捨てられたものの台帳の中に．
