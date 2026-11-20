; hello.asm — 背景に文字を表示する（第 7・8 章で使う）
;
; ・パレット、ネームテーブル、属性テーブルを書き込んで文字とブロックを表示する
; ・NMI のたびにゼロページの frame_count（$00）を 1 増やす
;
; ビルド: ../build.sh

PPUCTRL   = $2000
PPUMASK   = $2001
PPUSTATUS = $2002
PPUSCROLL = $2005
PPUADDR   = $2006
PPUDATA   = $2007

frame_count = $00       ; NMI が起きた回数

        .org $C000
reset:
        sei             ; IRQ を禁止する
        cld             ; 10 進モードを解除する（ファミコンの CPU には無いが習慣として）
        ldx #$FF
        txs             ; スタックポインタを $01FF に
        inx             ; X = 0
        stx PPUCTRL     ; NMI を止める
        stx PPUMASK     ; 描画を止める
        stx $4010       ; DMC の IRQ を止める
        lda #$40
        sta $4017       ; APU のフレーム IRQ を止める

        ; 電源投入直後の PPU は書き込みを受け付けないので、VBlank を 2 回待つ
        bit PPUSTATUS
vblank1:
        bit PPUSTATUS
        bpl vblank1

        ; RAM（$0000〜$07FF）を 0 にする
        lda #0
        tax
clear_ram:
        sta $0000,x
        sta $0100,x
        sta $0200,x
        sta $0300,x
        sta $0400,x
        sta $0500,x
        sta $0600,x
        sta $0700,x
        inx
        bne clear_ram

vblank2:
        bit PPUSTATUS
        bpl vblank2

        ; パレット（$3F00〜$3F1F）を書き込む
        lda PPUSTATUS   ; $2006 の書き込み順（上位→下位）をリセットする
        lda #$3F
        sta PPUADDR
        lda #$00
        sta PPUADDR
        ldx #0
load_palette:
        lda palette,x
        sta PPUDATA
        inx
        cpx #32
        bne load_palette

        ; ネームテーブル 0（$2000〜$23FF、属性テーブルを含む 1024 バイト）を 0 で埋める
        lda #$20
        sta PPUADDR
        lda #$00
        sta PPUADDR
        ldy #4
clear_nt:
        ldx #0
clear_nt_inner:
        sta PPUDATA
        dex
        bne clear_nt_inner
        dey
        bne clear_nt

        ; 文字列やブロックを書く（[アドレス上位, 下位, タイル..., 0] の並び。$FF で終わり）
        ldx #0
next_text:
        lda texts,x
        cmp #$FF
        beq texts_done
        sta PPUADDR
        inx
        lda texts,x
        sta PPUADDR
        inx
text_loop:
        lda texts,x
        inx
        cmp #0
        beq next_text
        sta PPUDATA
        jmp text_loop
texts_done:

        ; 属性テーブル（$23C0〜）でパレットを選ぶ
        ldx #0
next_attr:
        lda attributes,x
        cmp #$FF
        beq attr_done
        sta PPUADDR
        inx
        lda attributes,x
        sta PPUADDR
        inx
        lda attributes,x    ; 値
        inx
        ldy #8              ; 横 1 列ぶん（8 バイト）同じ値を書く
attr_loop:
        sta PPUDATA
        dey
        bne attr_loop
        jmp next_attr
attr_done:

        ; $2006 に書いたことで内部のスクロール位置が変わったので、0 に戻す
        lda #0
        sta PPUSCROLL
        sta PPUSCROLL
        lda #%10000000  ; NMI を有効に（ネームテーブル $2000、背景のパターンは $0000）
        sta PPUCTRL
        lda #%00001010  ; 背景を表示する（左端 8 ドットも表示）
        sta PPUMASK

forever:
        jmp forever

nmi:
        inc frame_count
        rti

irq:
        rti

palette:
        .byte $0F, $30, $16, $2A    ; 背景パレット 0: 白・赤・緑
        .byte $0F, $21, $11, $01    ; 背景パレット 1: 青
        .byte $0F, $27, $17, $07    ; 背景パレット 2: オレンジ
        .byte $0F, $34, $24, $14    ; 背景パレット 3: 紫
        .byte $0F, $30, $16, $2A    ; スプライトパレット 0〜3（この ROM では使わない）
        .byte $0F, $30, $16, $2A
        .byte $0F, $30, $16, $2A
        .byte $0F, $30, $16, $2A

; ネームテーブルのアドレス = $2000 + 行 * 32 + 列
texts:
        .byte $21, $6A, "HELLO, NES!", 0                    ; 11 行 10 列
        .byte $21, $A9, "MYNES EMULATOR", 0                 ; 13 行 9 列
        .byte $22, $06, "BG+PALETTE+ATTRIBUTE", 0           ; 16 行 6 列
        .byte $22, $88, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 0   ; 20 行
        .byte $22, $C8, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 0   ; 22 行
        .byte $FF

; 属性テーブルは 32x32 ドット（4x4 タイル）ごとに 1 バイト。
; 下位ビットから 左上・右上・左下・右下 の 16x16 ドットのパレット番号（2 ビットずつ）
attributes:
        .byte $23, $D8, %00000101   ; 12〜15 行: 上半分（12・13 行）をパレット 1
        .byte $23, $E0, %00001010   ; 16〜19 行: 上半分（16・17 行）をパレット 2
        .byte $23, $E8, %11110000   ; 20〜23 行: 下半分（22・23 行）をパレット 3
        .byte $FF

        .org $FFFA
        .word nmi, reset, irq
