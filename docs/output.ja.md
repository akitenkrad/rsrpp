[English](output.md) | **日本語**

# 出力フォーマット

```json
[
  {
    "index": 0,
    "title": "Abstract",
    "contents": ["This paper presents...", "Our approach achieves..."],
    "math_contents": ["This paper presents...", "Our approach achieves α = 0.95..."],
    "captions": []
  },
  {
    "index": 1,
    "title": "1 Introduction",
    "contents": ["Deep learning has..."],
    "math_contents": ["Deep learning has <math>f(x) = Wx + b</math>..."],
    "captions": ["Figure 1: Overview of our proposed method."]
  }
]
```

## セクションのフィールド

| フィールド | 型 | 説明 |
|-------|-----|------|
| `index` | `i16` | 文書中での節の順序 |
| `title` | `String` | 節のタイトル（例: "Abstract", "1 Introduction"） |
| `contents` | `Vec<String>` | 元のテキスト（キャプションを除く） |
| `math_contents` | `Option<Vec<String>>` | 数式を `<math>...</math>` タグで囲んだテキスト（数式が検出された場合のみ存在する） |
| `captions` | `Vec<String>` | この節に属する図表のキャプション（無ければ空配列） |

## 数式のマークアップ

数式の検出は多層的な方法で行われる:

1. **LLM 経路**（`OPENAI_API_KEY` が設定されている場合）: 視覚 LLM で数式を抽出し，trigram
   による曖昧一致で個々のテキストブロックへ対応付ける
2. **ヒューリスティック経路**（フォールバック）: パターンによる検出と文脈の解析

検出されるパターンには次のものが含まれる:

- ギリシャ文字（α, β, γ など）
- 数学記号（∑, ∏, ∫, ≤, ≥, ∈ など）
- 複数文字からなる数学関数（`sin(x)`, `cos(θ)`, `log(n)` など）
- ASCII の指数・添字（`x^2`, `x_i`, `a_{n+1}`）
- 文字どうしの分数（`a/b`）
- ノルムの記法（`||w||`）
- よくある数式のパターン（文脈にもとづく検証つき）

誤検出のフィルタが除外するもの:

- 日付のパターン（`2019/2020`）
- 統計の報告（`n = 50 participants`）
- 節・図の参照（`Section 3.1`, `Figure 2`）

数式の出力はすべて `<math>...</math>` タグの中で **LaTeX 形式** に統一される:

```
Original: "The learning rate α converges when x^2 ≤ β"
Marked:   "The learning rate <math>\alpha</math> converges when <math>x^2 \leq \beta</math>"
```

ライブラリからマークアップ済みの形を得るには，パースが埋めた数式マップを渡して節を組み立てる:

```rust
let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);
```

## 出力に含まれないもの

検出された表領域の中のテキストと，本文としては細すぎ短すぎるブロックは，意図的に除外される．
とはいえ捨て去られるわけではない．どのパースも報告する数値については
[テキストカバレッジ](cli.ja.md#テキストカバレッジ) を，除外された断片を取り戻す方法については
`--keep-dropped` を参照．
