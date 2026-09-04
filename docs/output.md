**English** | [日本語](output.ja.md)

# Output format

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

## Section fields

| Field | Type | Description |
|-------|------|-------------|
| `index` | `i16` | Section order in document |
| `title` | `String` | Section title (e.g., "Abstract", "1 Introduction") |
| `contents` | `Vec<String>` | Original text content (captions excluded) |
| `math_contents` | `Option<Vec<String>>` | Text with math expressions wrapped in `<math>...</math>` tags (only present if math detected) |
| `captions` | `Vec<String>` | Figure/table captions belonging to this section (empty array if none) |

## Math markup

Math expressions are detected using a multi-layered approach:

1. **LLM path** (when `OPENAI_API_KEY` is set): Extracts math via vision LLM, then aligns results to individual text blocks using trigram-based fuzzy matching
2. **Heuristic path** (fallback): Pattern-based detection with context analysis

Detected patterns include:

- Greek letters (α, β, γ, etc.)
- Mathematical operators (∑, ∏, ∫, ≤, ≥, ∈, etc.)
- Multi-character math functions (`sin(x)`, `cos(θ)`, `log(n)`, etc.)
- ASCII exponents and subscripts (`x^2`, `x_i`, `a_{n+1}`)
- Letter fractions (`a/b`)
- Norm notation (`||w||`)
- Common equation patterns with context-based validation

False positive filtering excludes:

- Date patterns (`2019/2020`)
- Statistical reporting (`n = 50 participants`)
- Section/figure references (`Section 3.1`, `Figure 2`)

All math output is unified to **LaTeX format** inside `<math>...</math>` tags:

```
Original: "The learning rate α converges when x^2 ≤ β"
Marked:   "The learning rate <math>\alpha</math> converges when <math>x^2 \leq \beta</math>"
```

To get the marked-up form from the library, build the sections with the math map the
parse filled in:

```rust
let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);
```

## What is not in the output

Text inside a detected table region, and blocks too narrow and short to be body text,
are excluded on purpose. They are not thrown away: see
[Text coverage](cli.md#text-coverage) for the number every parse reports, and
`--keep-dropped` for getting the excluded fragments back.
