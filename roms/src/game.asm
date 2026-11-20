; game.asm — スプライト、コントローラー、音（第 10〜12 章で使う）
;
; ・16x16 のキャラクター（スプライト 4 個）を十字キーで動かす。左に動くと水平反転する
; ・ボールが画面の中を跳ね回る
; ・星を横一列に 10 個並べる（1 ラインに表示できるスプライトは 8 個まで）
; ・スプライトは RAM の $0200〜$02FF に作り、NMI の中で OAM DMA で転送する
; ・矩形波 1 でメロディー、三角波でベース、ノイズでハイハットを鳴らす（8 フレームで 1 ステップ、16 ステップでくり返し）
; ・A ボタン: 矩形波 2 のスイープでジャンプ音、B ボタン: ノイズで爆発音
;
; ビルド: ../build.sh

PPUCTRL   = $2000
PPUMASK   = $2001
PPUSTATUS = $2002
OAMADDR   = $2003
PPUSCROLL = $2005
PPUADDR   = $2006
PPUDATA   = $2007
OAMDMA    = $4014
JOYPAD1   = $4016

SQ1_VOL    = $4000
SQ1_SWEEP  = $4001
SQ1_LO     = $4002
SQ1_HI     = $4003
SQ2_VOL    = $4004
SQ2_SWEEP  = $4005
SQ2_LO     = $4006
SQ2_HI     = $4007
TRI_LINEAR = $4008
TRI_LO     = $400A
TRI_HI     = $400B
NOISE_VOL  = $400C
NOISE_LO   = $400E
NOISE_HI   = $400F
SND_CHN    = $4015

; read_joypad が作る buttons のビット
BUTTON_A      = %10000000
BUTTON_B      = %01000000
BUTTON_SELECT = %00100000
BUTTON_START  = %00010000
BUTTON_UP     = %00001000
BUTTON_DOWN   = %00000100
BUTTON_LEFT   = %00000010
BUTTON_RIGHT  = %00000001

frame_count = $00       ; NMI が起きた回数
buttons     = $01       ; 押されているボタン
player_x    = $02
player_y    = $03
facing      = $04       ; 0 = 右向き、$40 = 左向き（スプライト属性の水平反転ビット）
ball_x      = $05
ball_y      = $06
ball_dx     = $07       ; 1 か -1（$FF）
ball_dy     = $08
prev_buttons = $09
pressed     = $0A       ; このフレームで新しく押されたボタン
step        = $0B       ; 曲のステップ（0〜15）
tick        = $0C       ; 次のステップまでのフレーム数
noise_busy  = $0D       ; 爆発音が鳴っている間はハイハットを鳴らさない
oam         = $0200     ; OAM に転送するバッファ（スプライト 64 個 × 4 バイト）

; 音の番号（period_lo / period_hi の添字 + 1）。0 は休符
G3 = 8
A3 = 10
B3 = 12
C4 = 13
D4 = 15
E4 = 17
F4 = 18
G4 = 20
A4 = 22
B4 = 24
C5 = 25
D5 = 27
E5 = 29
F5 = 30
G5 = 32
A5 = 34

        .org $C000
reset:
        sei
        cld
        ldx #$FF
        txs
        inx
        stx PPUCTRL
        stx PPUMASK
        stx $4010
        lda #$40
        sta $4017

        bit PPUSTATUS
vblank1:
        bit PPUSTATUS
        bpl vblank1

        lda #0
        tax
clear_ram:
        sta $0000,x
        sta $0100,x
        sta $0300,x
        sta $0400,x
        sta $0500,x
        sta $0600,x
        sta $0700,x
        lda #$FF        ; OAM バッファは $FF（Y 座標が画面外）で埋めてスプライトを隠す
        sta oam,x
        lda #0
        inx
        bne clear_ram

vblank2:
        bit PPUSTATUS
        bpl vblank2

        ; パレット
        lda PPUSTATUS
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

        ; ネームテーブル 0 を消す
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

        ; 文字
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

        ; 地面のレンガ（27〜29 行 = $2360 から 96 タイル）
        lda #$23
        sta PPUADDR
        lda #$60
        sta PPUADDR
        lda #$04
        ldx #96
ground_loop:
        sta PPUDATA
        dex
        bne ground_loop

        ; 地面の属性（パレット 2）: 24〜27 行の下半分と 28〜31 行の上半分
        lda #$23
        sta PPUADDR
        lda #$F0
        sta PPUADDR
        ldx #8
        lda #%10100000
attr1:
        sta PPUDATA
        dex
        bne attr1
        ldx #8
        lda #%00001010
attr2:
        sta PPUDATA
        dex
        bne attr2

        ; 変数の初期値
        lda #120
        sta player_x
        lda #136
        sta player_y
        lda #40
        sta ball_x
        lda #60
        sta ball_y
        lda #1
        sta ball_dx
        sta ball_dy

        ; 星を 10 個、同じ高さに並べる（スプライト 5〜14）
        ldx #0
        ldy #0
star_loop:
        lda #175
        sta oam+20,x    ; Y（スプライトは 1 ライン下に表示されるので 176 ライン目から）
        lda #$02
        sta oam+21,x    ; タイル
        lda #$02
        sta oam+22,x    ; 属性（パレット 2）
        lda star_x,y
        sta oam+23,x    ; X
        inx
        inx
        inx
        inx
        iny
        cpy #10
        bne star_loop

        jsr build_oam

        ; APU: 矩形波 1・2、三角波、ノイズを有効にする
        lda #%00001111
        sta SND_CHN

        lda #0
        sta PPUSCROLL
        sta PPUSCROLL
        lda #%10001000  ; NMI 有効、スプライトのパターンは $1000
        sta PPUCTRL
        lda #%00011110  ; 背景とスプライトを表示（左端 8 ドットも表示）
        sta PPUMASK

main_loop:
        ; NMI が来る（frame_count が変わる）まで待つ
        lda frame_count
wait_frame:
        cmp frame_count
        beq wait_frame

        jsr read_joypad
        lda prev_buttons
        eor #$FF
        and buttons         ; 前のフレームで押されていなかったボタンだけ残す
        sta pressed
        lda buttons
        sta prev_buttons

        lda buttons
        and #BUTTON_RIGHT
        beq not_right
        inc player_x
        lda #0
        sta facing
not_right:
        lda buttons
        and #BUTTON_LEFT
        beq not_left
        dec player_x
        lda #$40
        sta facing
not_left:
        lda buttons
        and #BUTTON_UP
        beq not_up
        dec player_y
not_up:
        lda buttons
        and #BUTTON_DOWN
        beq not_down
        inc player_y
not_down:
        lda pressed
        and #BUTTON_A
        beq no_a
        jsr play_jump
no_a:
        lda pressed
        and #BUTTON_B
        beq no_b
        jsr play_explosion
no_b:
        jsr move_ball
        jsr build_oam
        jsr play_music
        jmp main_loop

; 8 フレームごとに曲を 1 ステップ進める
play_music:
        lda noise_busy
        beq noise_free
        dec noise_busy
noise_free:
        dec tick
        bpl music_done
        lda #7
        sta tick
        jsr play_step
        lda step
        clc
        adc #1
        and #15
        sta step
music_done:
        rts

; ステップ step の音を鳴らす
play_step:
        ldx step
        lda melody,x
        beq no_melody
        tay                 ; Y = 音の番号
        lda #%01000100      ; デューティ 25%、エンベロープで減衰（速さ 4）
        sta SQ1_VOL
        lda #%00001000      ; スイープなし（negate を立てておかないと低い音が鳴らない）
        sta SQ1_SWEEP
        lda period_lo-1,y
        sta SQ1_LO
        lda period_hi-1,y
        ora #%00001000      ; 長さカウンタ（インデックス 1 = 254）
        sta SQ1_HI
no_melody:
        lda bass,x
        beq no_bass
        tay
        lda #%00100000      ; 線形カウンタ 32（約 8 フレームで止まる）
        sta TRI_LINEAR
        lda period_lo-1,y   ; 三角波は同じ値で 1 オクターブ低い音になる
        sta TRI_LO
        lda period_hi-1,y
        ora #%00001000
        sta TRI_HI
no_bass:
        txa
        and #1
        bne no_hat          ; ハイハットは偶数ステップだけ
        lda noise_busy
        bne no_hat
        lda #%00000001      ; エンベロープで減衰（とても速い）
        sta NOISE_VOL
        lda #$03            ; 周期の番号 3（高い音）
        sta NOISE_LO
        lda #%00001000
        sta NOISE_HI
no_hat:
        rts

; ジャンプ音: 矩形波 2 のスイープで音程を上げていく
play_jump:
        lda #%10000110      ; デューティ 50%、エンベロープで減衰（速さ 6）
        sta SQ2_VOL
        lda #%10111011      ; スイープ有効、周期 3、negate（周期が短くなる = 音が高くなる）、シフト 3
        sta SQ2_SWEEP
        lda #$AB            ; 周期 $1AB（ド）から
        sta SQ2_LO
        lda #%00001001
        sta SQ2_HI
        rts

; 爆発音: 低いノイズをゆっくり減衰させる
play_explosion:
        lda #%00001000      ; エンベロープで減衰（速さ 8）
        sta NOISE_VOL
        lda #$0C            ; 周期の番号 12（低い音）
        sta NOISE_LO
        lda #%00001000
        sta NOISE_HI
        lda #40
        sta noise_busy
        rts

; コントローラー 1 を読んで buttons に入れる（ビット 7 から A, B, SELECT, START, 上, 下, 左, 右）
read_joypad:
        lda #1
        sta JOYPAD1     ; ストローブ ON: ボタンの状態を取り込む
        sta buttons     ; 番兵: 8 回シフトするとこの 1 がキャリーに出てくる
        lsr a
        sta JOYPAD1     ; ストローブ OFF: 1 ビットずつ読み出せるようになる
joy_loop:
        lda JOYPAD1
        lsr a           ; ビット 0 → キャリー
        rol buttons     ; キャリー → buttons のビット 0
        bcc joy_loop
        rts

; ボールを動かし、端で跳ね返す
move_ball:
        lda ball_x
        clc
        adc ball_dx
        sta ball_x
        cmp #240
        bcs flip_dx
        cmp #8
        bcs x_ok
flip_dx:
        lda #0
        sec
        sbc ball_dx
        sta ball_dx
x_ok:
        lda ball_y
        clc
        adc ball_dy
        sta ball_y
        cmp #200
        bcs flip_dy
        cmp #24
        bcs y_ok
flip_dy:
        lda #0
        sec
        sbc ball_dy
        sta ball_dy
y_ok:
        rts

; プレイヤーとボールを OAM バッファに書く
build_oam:
        ldx #0          ; OAM バッファのオフセット
        ldy #0          ; 部品の番号（0〜3）
player_loop:
        lda player_y
        clc
        adc part_dy,y
        sta oam,x       ; Y 座標
        lda facing
        bne face_left
        lda tile_right,y
        jmp store_tile
face_left:
        lda tile_left,y
store_tile:
        sta oam+1,x     ; タイル番号
        lda facing
        sta oam+2,x     ; 属性（パレット 0 + 反転ビット）
        lda player_x
        clc
        adc part_dx,y
        sta oam+3,x     ; X 座標
        inx
        inx
        inx
        inx
        iny
        cpy #4
        bne player_loop

        lda ball_y
        sta oam+16
        lda #$01
        sta oam+17
        lda #$01        ; パレット 1
        sta oam+18
        lda ball_x
        sta oam+19
        rts

nmi:
        pha
        lda #0
        sta OAMADDR
        lda #>oam
        sta OAMDMA      ; $0200〜$02FF を OAM へコピーする
        inc frame_count
        pla
        rti

irq:
        rti

part_dx:    .byte 0, 8, 0, 8
part_dy:    .byte 0, 0, 8, 8
tile_right: .byte $10, $11, $12, $13
tile_left:  .byte $11, $10, $13, $12     ; 左右の部品を入れ替えて、それぞれを反転する
star_x:     .byte 16, 40, 64, 88, 112, 136, 160, 184, 208, 232

; 16 ステップの曲（C → F → G → C）
melody: .byte C5, E5, G5, E5,  C5, F5, A5, F5,  B4, D5, G5, D5,  C5, E5, G5, 0
bass:   .byte C4, 0, C4, 0,    F4, 0, F4, 0,    G4, 0, G4, 0,    C4, 0, G3, 0

; 矩形波のタイマー周期（C3〜B5）。周期 = 1789773 / (16 × 周波数) - 1
period_lo:
        .byte $56, $26, $F9, $CE, $A6, $80, $5C, $3A, $1A, $FB, $DF, $C4
        .byte $AB, $93, $7C, $67, $52, $3F, $2D, $1C, $0C, $FD, $EF, $E1
        .byte $D5, $C9, $BD, $B3, $A9, $9F, $96, $8E, $86, $7E, $77, $70
period_hi:
        .byte $03, $03, $02, $02, $02, $02, $02, $02, $02, $01, $01, $01
        .byte $01, $01, $01, $01, $01, $01, $01, $01, $01, $00, $00, $00
        .byte $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00, $00

palette:
        .byte $0F, $30, $10, $00    ; 背景 0: 文字
        .byte $0F, $21, $11, $01
        .byte $0F, $27, $17, $0F    ; 背景 2: レンガ
        .byte $0F, $30, $10, $00
        .byte $0F, $28, $16, $30    ; スプライト 0: キャラクター
        .byte $0F, $21, $11, $30    ; スプライト 1: ボール
        .byte $0F, $38, $28, $18    ; スプライト 2: 星
        .byte $0F, $30, $10, $00

texts:
        .byte $20, $4A, "GAME DEMO", 0               ; 2 行 10 列
        .byte $20, $8A, "D-PAD: MOVE", 0             ; 4 行 10 列
        .byte $20, $CA, "A: JUMP  B: BOOM", 0        ; 6 行 10 列
        .byte $22, $85, "10 STARS ON ONE LINE", 0    ; 20 行 5 列
        .byte $FF

        .org $FFFA
        .word nmi, reset, irq
