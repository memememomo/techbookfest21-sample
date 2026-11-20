# Re:ゼロからAIに教わりながらRustでミニエミュレーターを実装したらブラウザ上でファミコン風ゲームが動き出した件 サンプルコード

技術書典21 の新刊『Re:ゼロからAIに教わりながらRustでミニエミュレーターを実装したらブラウザ上でファミコン風ゲームが動き出した件』のサンプルコードです。

## 中身

```
.
├── ch03/ 〜 ch12/   各章を終えた時点の完成コード（mynes）
├── roms/            本書で使うデモ ROM（hello.nes、game.nes）
│   ├── src/         デモ ROM のソース（hello.asm、game.asm、タイルの絵を作る make_chr.py）
│   └── build.sh     デモ ROM をソースから作り直す
├── nesasm/          デモ ROM を作るアセンブラ
├── viewer/          デモ ROM の中身（パターンテーブル、ネームテーブルなど）を見る ROM ビューアー
├── appc/            付録 C（ビット演算）の課題の解答例
└── tools/verify.sh  すべての章のビルドとテストを確かめる
```

付録は [Zenn の本](https://zenn.dev/uchiko/books/rezero-rust-nes-appendix)で公開しています。

## 使い方

各章の完成コードは、そのままビルドしてテストできます。
各章の `roms/` は、リポジトリ直下の `roms/` へのシンボリックリンクです。

```console
$ cd ch08
$ cargo test
$ cargo run -- info roms/hello.nes
```

自分で書いているプロジェクトでは、デモ ROM をプロジェクトの `roms/` にコピーしてください（第 1 章）。

```console
$ mkdir roms
$ cp /path/to/techbookfest21-sample/roms/*.nes roms/
```

第 9 章からは、ブラウザで動かせます。

```console
$ rustup target add wasm32-unknown-unknown
$ cargo install wasm-pack
$ cd ch12
$ wasm-pack build --target web --out-dir web/pkg
$ python3 -m http.server 8000 --directory web
```

ブラウザで http://localhost:8000/ を開き、「ファイルを選択」で `roms/game.nes` を選びます。

### Windows で使うとき

各章の `roms/` はシンボリックリンクなので、Windows では Git の設定によってはただのファイルになります。
その場合は、`roms/` を消してリポジトリ直下の `roms/` をコピーしてください。
ROM が無いときは、ROM を使うテストは「〜が無いので飛ばします」と表示して通ります。

## ROM の中身を見る

`viewer/` は、デモ ROM を動かしながら、パターンテーブル・ネームテーブル・属性テーブル・パレット・OAM を絵と表で見せる ROM ビューアーです。
タイルにマウスを乗せると、どのアドレスのどのビットから色が決まるかをたどれます。

```console
$ cd viewer
$ wasm-pack build --target web --out-dir web/pkg
$ python3 -m http.server 8000
```

ブラウザで http://localhost:8000/web/ を開き、「hello.nes」か「game.nes」のボタンを押します。
詳しくは [viewer/README.md](viewer/README.md) を見てください。

## デモ ROM を作り直す

`roms/build.sh` を実行すると、2 本のデモ ROM をソースから作り直します。Rust と Python 3 が必要です。

```console
$ roms/build.sh
Wrote hello.nes (NROM, PRG 16384 B, CHR 8192 B)
Wrote game.nes (NROM, PRG 16384 B, CHR 8192 B)
```

アセンブラの使い方は [nesasm/README.md](nesasm/README.md) を見てください。

## 確かめる

```console
$ tools/verify.sh            # すべて
$ tools/verify.sh 8 9        # 第 8 章と第 9 章だけ
$ tools/verify.sh viewer     # ROM ビューアーだけ
$ SKIP_WASM=1 tools/verify.sh  # WebAssembly のビルドを省く
```

## ROM について

このリポジトリのデモ ROM は、本書のために書いたものです。
市販のゲームの ROM を、インターネットからダウンロードしたり配ったりしてはいけません。
