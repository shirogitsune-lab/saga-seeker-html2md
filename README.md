# saga-seeker-html2md v1.2.0

Saga & SeekerのHTMLキャラクターシートから必要な情報だけを抜き出し、ChatGPTやCodexに読ませやすいMarkdownファイルとして出力するRust製ツールです。

このツールは個人制作の非公式ツールであり、Saga & Seeker公式とは関係ありません。

## 使い方

1. `saga-seeker-html2md.exe` と同じフォルダにある `input` フォルダへ、変換したい `.html` または `.htm` ファイルを入れます。
2. `saga-seeker-html2md.exe` をダブルクリックします。
3. 処理が終わったら、`Enterキーで終了します...` と表示されるので Enter キーを押します。
4. `output` フォルダに、入力HTMLと同名の `.md` ファイルが出力されます。

詳しい処理結果やエラー内容は、同じフォルダに作成される `log.txt` を確認してください。

`input` フォルダや `output` フォルダがない場合は、自動で作成されます。
`input` にHTMLファイルがない場合も異常終了せず、案内メッセージを表示して終了します。

## Windowsの警告について

このリリースの実行ファイルには自己署名のAuthenticode署名を付与していますが、Windows SmartScreenの警告が表示される場合があります。
この自己署名は改ざん確認の補助であり、SmartScreen警告を回避するものではありません。
配布元が提供している `SHA256SUMS.txt` と手元のファイルのSHA256値を照合して、ファイルが改変されていないことを確認することを推奨します。

通常の利用手順として、Windows Defenderの除外設定やタスクスケジューラ登録は推奨しません。

## Source版の内容物

```text
saga-seeker-html2md-v1.2.0-source/
├─ .cargo/
│  └─ config.toml
├─ src/
│  └─ main.rs
├─ samples/
│  ├─ sample.html
│  └─ expected.md
├─ input/
│  └─ .gitkeep
├─ output/
│  └─ .gitkeep
├─ Cargo.toml
├─ Cargo.lock
├─ README.md
├─ SECURITY_NOTICE.md
├─ CHANGELOG.md
├─ LICENSE
├─ THIRD_PARTY_NOTICES.txt
└─ SHA256SUMS.txt
```

Source版には、`target/`、ビルド済みexe、実キャラクターHTML、変換済みMarkdown、`log.txt` は含めません。

## 開発者向けビルド

必要環境:

- Rust stable
- Cargo
- Windows向けにビルドする場合は、Visual Studio Build Tools のC++ビルド環境

Windows向けrelease buildでは、`.cargo/config.toml` により `x86_64-pc-windows-msvc` の `crt-static` を有効化し、配布exeの依存関係を減らします。

```powershell
cargo fmt --check
cargo check
cargo build --release
```

ビルド後、実行ファイルは以下に生成されます。

```text
target\release\saga-seeker-html2md.exe
```

## 実行テスト

このツールは、実行ファイルが置かれているフォルダを基準に `input` / `output` / `log.txt` を扱います。
プロジェクト直下で配布時に近い挙動を確認する場合は、次のように実行します。

```powershell
cargo build --release
Copy-Item ".\target\release\saga-seeker-html2md.exe" ".\saga-seeker-html2md.exe" -Force
Copy-Item ".\samples\sample.html" ".\input\sample.html" -Force
Write-Output "" | & ".\saga-seeker-html2md.exe"
Compare-Object `
  (Get-Content ".\samples\expected.md" -Encoding UTF8) `
  (Get-Content ".\output\sample.md" -Encoding UTF8)
```

`Compare-Object` で差分が出なければ、サンプル変換結果は期待値と一致しています。

## 実装方針

- `input` フォルダ直下の `.html` / `.htm` を対象にします。
- UTF-8を優先し、読めない場合はShift_JISとして読み込みます。
- `kuchiki` でHTMLを1回だけDOM化し、スキル抽出と不要要素除去を同じDOM上で行います。
- `script[type="application/json"]` 由来のスキルは、`script` 要素を削除する前に抽出します。
- `html-to-markdown-rs` でMarkdown化します。
- Markdown化後、SAGA & SEEKER向けの後処理を行います。

## SAGA & SEEKER向けの主な後処理

- `## キャラクター詳細` 配下の `基本設定`、`外見`、`性格`、`口調`、`経歴`、`特技と役割`、`その他の特徴` を `###` 見出しに変換。
- スキルはHTML属性または内部JSONの `skills` から、スキル名と説明文だけを抽出。
- `type` が空でも、独自スキル・シナリオ報酬スキルとして有効扱いします。
- `type` はMarkdownに出力しません。
- `id` の形式だけではスキルを破棄しません。
- ステータスは `筋力`、`耐久力`、`知力`、`精神力`、`素早さ`、`運` の6項目のみ出力します。
- `魅力` は没データとして、ステータス・本文項目・スキル名として出力しません。
- ただし、通常の文章中に出てくる「魅力」という単語は削除対象ではありません。

## 配布物の作成方針

一般利用者向けには、以下だけを含めたアーカイブを作成してください。

```text
saga-seeker-html2md-v1.2.0-windows-x64/
├─ saga-seeker-html2md.exe
├─ input/
│  └─ .gitkeep
├─ output/
│  └─ .gitkeep
├─ README.md
├─ SECURITY_NOTICE.md
├─ CHANGELOG.md
├─ LICENSE
├─ THIRD_PARTY_NOTICES.txt
└─ SHA256SUMS.txt
```

開発者向けには、このSource版のようにソース一式を含め、`target/` や実データは含めないでください。

## ライセンス

このツール本体のライセンスは `LICENSE` を参照してください。
利用しているRust crateについては `THIRD_PARTY_NOTICES.txt` を参照してください。
