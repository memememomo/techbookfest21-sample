//! APU（音を作る部分）: 矩形波 2 つ、三角波、ノイズ
//!
//! 本物にはもう 1 つ DMC（サンプリング音）があるが、この教材では省略する。
//!
//! 各チャンネルは「タイマー」で決まる周期で波形を進め、「長さカウンタ」や「エンベロープ」で
//! 音の長さと音量を決める。これらはフレームシーケンサーが 1/240 秒ごとに動かす。

/// CPU のクロック（NTSC）
pub const CPU_CLOCK: f64 = 1_789_773.0;

/// 長さカウンタに入れる値（レジスタに書くのはこの表の番号）
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

/// 矩形波の形（デューティ比 12.5%, 25%, 50%, 75%）
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

/// 三角波の形（15 → 0 → 15 の 32 段階）
const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

/// ノイズの周期（CPU サイクル）
const NOISE_PERIOD: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// エンベロープ: 音量をだんだん小さくする（または一定の音量にする）
#[derive(Default)]
struct Envelope {
    start: bool,
    divider: u8,
    decay: u8,
    /// 一定音量のときは音量、そうでなければ減衰の速さ
    period: u8,
    looped: bool,
    constant: bool,
}

impl Envelope {
    fn write(&mut self, value: u8) {
        self.looped = value & 0x20 != 0;
        self.constant = value & 0x10 != 0;
        self.period = value & 0x0F;
    }

    /// 1/240 秒ごと
    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.period;
        } else if self.divider > 0 {
            self.divider -= 1;
        } else {
            self.divider = self.period;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looped {
                self.decay = 15;
            }
        }
    }

    fn volume(&self) -> u8 {
        if self.constant {
            self.period
        } else {
            self.decay
        }
    }
}

/// 長さカウンタ: 0 になったら音を止める
#[derive(Default)]
struct LengthCounter {
    value: u8,
    halt: bool,
    enabled: bool,
}

impl LengthCounter {
    fn load(&mut self, index: u8) {
        if self.enabled {
            self.value = LENGTH_TABLE[index as usize];
        }
    }

    /// 1/120 秒ごと
    fn clock(&mut self) {
        if !self.halt && self.value > 0 {
            self.value -= 1;
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.value = 0;
        }
    }
}

/// 矩形波
#[derive(Default)]
struct Pulse {
    /// 1 か 2（スイープの計算が少し違う）
    channel: u8,
    duty: u8,
    step: u8,
    timer: u16,
    period: u16,
    envelope: Envelope,
    length: LengthCounter,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_divider: u8,
    sweep_reload: bool,
}

impl Pulse {
    fn new(channel: u8) -> Self {
        Pulse {
            channel,
            ..Default::default()
        }
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.duty = value >> 6;
                self.length.halt = value & 0x20 != 0;
                self.envelope.write(value);
            }
            1 => {
                self.sweep_enabled = value & 0x80 != 0;
                self.sweep_period = (value >> 4) & 0x07;
                self.sweep_negate = value & 0x08 != 0;
                self.sweep_shift = value & 0x07;
                self.sweep_reload = true;
            }
            2 => self.period = (self.period & 0x0700) | value as u16,
            _ => {
                self.period = (self.period & 0x00FF) | ((value as u16 & 0x07) << 8);
                self.length.load(value >> 3);
                self.step = 0;
                self.envelope.start = true;
            }
        }
    }

    /// CPU 2 サイクルごと
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.step = (self.step + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    /// スイープで次に設定される周期
    fn sweep_target(&self) -> u16 {
        let change = self.period >> self.sweep_shift;
        if self.sweep_negate {
            // 矩形波 1 は、さらに 1 小さくなる
            let extra = if self.channel == 1 { 1 } else { 0 };
            self.period.saturating_sub(change + extra)
        } else {
            self.period + change
        }
    }

    /// 1/120 秒ごと
    fn clock_sweep(&mut self) {
        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0 && !self.muted() {
            self.period = self.sweep_target();
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    /// 周期が短すぎる・長すぎるときは音が出ない（スイープが無効でも）
    fn muted(&self) -> bool {
        self.period < 8 || self.sweep_target() > 0x07FF
    }

    fn output(&self) -> u8 {
        if self.length.value == 0
            || self.muted()
            || DUTY_TABLE[self.duty as usize][self.step as usize] == 0
        {
            0
        } else {
            self.envelope.volume()
        }
    }
}

/// 三角波
#[derive(Default)]
struct Triangle {
    step: u8,
    timer: u16,
    period: u16,
    length: LengthCounter,
    linear_counter: u8,
    linear_reload_value: u8,
    linear_reload: bool,
    control: bool,
}

impl Triangle {
    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.control = value & 0x80 != 0;
                self.length.halt = self.control;
                self.linear_reload_value = value & 0x7F;
            }
            2 => self.period = (self.period & 0x0700) | value as u16,
            3 => {
                self.period = (self.period & 0x00FF) | ((value as u16 & 0x07) << 8);
                self.length.load(value >> 3);
                self.linear_reload = true;
            }
            _ => {}
        }
    }

    /// CPU 1 サイクルごと
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            // 長さカウンタと線形カウンタの両方が 0 でないときだけ波形を進める
            if self.length.value > 0 && self.linear_counter > 0 {
                self.step = (self.step + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }

    /// 1/240 秒ごと
    fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    fn output(&self) -> u8 {
        TRIANGLE_TABLE[self.step as usize]
    }
}

/// ノイズ
struct Noise {
    timer: u16,
    period: u16,
    /// 15 ビットのシフトレジスタ（疑似乱数）
    shift: u16,
    envelope: Envelope,
    length: LengthCounter,
}

impl Noise {
    fn new() -> Self {
        Noise {
            timer: 0,
            period: NOISE_PERIOD[0],
            shift: 1,
            envelope: Envelope::default(),
            length: LengthCounter::default(),
        }
    }

    fn write(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                self.length.halt = value & 0x20 != 0;
                self.envelope.write(value);
            }
            2 => self.period = NOISE_PERIOD[(value & 0x0F) as usize],
            3 => {
                self.length.load(value >> 3);
                self.envelope.start = true;
            }
            _ => {}
        }
    }

    /// CPU 1 サイクルごと
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period - 1;
            let feedback = (self.shift ^ (self.shift >> 1)) & 1;
            self.shift = (self.shift >> 1) | (feedback << 14);
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.length.value == 0 || self.shift & 1 != 0 {
            0
        } else {
            self.envelope.volume()
        }
    }
}

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    /// CPU サイクル数（矩形波は 2 サイクルに 1 回進むので、偶数か奇数かを見る）
    cycle: u64,
    /// フレームシーケンサーの位置（CPU サイクル）
    frame_cycle: u32,

    /// 出力するサンプルレート
    sample_rate: f64,
    sample_timer: f64,
    sample_sum: f32,
    sample_count: u32,
    filter_in: f32,
    filter_out: f32,
    /// できあがったサンプル（-1.0〜1.0 くらい）
    pub samples: Vec<f32>,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            pulse1: Pulse::new(1),
            pulse2: Pulse::new(2),
            triangle: Triangle::default(),
            noise: Noise::new(),
            cycle: 0,
            frame_cycle: 0,
            sample_rate: 44_100.0,
            sample_timer: 0.0,
            sample_sum: 0.0,
            sample_count: 0,
            filter_in: 0.0,
            filter_out: 0.0,
            samples: Vec::new(),
        }
    }

    pub fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
    }

    /// $4000〜$400F, $4015 への書き込み
    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr {
            0x4000..=0x4003 => self.pulse1.write(addr - 0x4000, value),
            0x4004..=0x4007 => self.pulse2.write(addr - 0x4004, value),
            0x4008..=0x400B => self.triangle.write(addr - 0x4008, value),
            0x400C..=0x400F => self.noise.write(addr - 0x400C, value),
            0x4015 => {
                // 各チャンネルを有効・無効にする（無効にすると長さカウンタが 0 になる）
                self.pulse1.length.set_enabled(value & 0x01 != 0);
                self.pulse2.length.set_enabled(value & 0x02 != 0);
                self.triangle.length.set_enabled(value & 0x04 != 0);
                self.noise.length.set_enabled(value & 0x08 != 0);
            }
            _ => {} // DMC（$4010〜$4013）などは省略
        }
    }

    /// CPU 1 サイクル分進める
    pub fn step(&mut self) {
        self.step_frame_counter();
        self.triangle.clock_timer();
        self.noise.clock_timer();
        if self.cycle % 2 == 1 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
        }
        self.cycle += 1;

        // CPU サイクルごとの出力を平均して、サンプルレートに合わせて取り出す
        self.sample_sum += self.mix();
        self.sample_count += 1;
        self.sample_timer += self.sample_rate / CPU_CLOCK;
        if self.sample_timer >= 1.0 {
            self.sample_timer -= 1.0;
            let sample = self.sample_sum / self.sample_count as f32;
            self.sample_sum = 0.0;
            self.sample_count = 0;
            // 直流成分を取り除く（ハイパスフィルタ）
            let out = sample - self.filter_in + 0.996 * self.filter_out;
            self.filter_in = sample;
            self.filter_out = out;
            self.samples.push(out);
        }
    }

    /// フレームシーケンサー: 約 1/240 秒ごとにエンベロープなどを、1/120 秒ごとに長さカウンタなどを動かす
    fn step_frame_counter(&mut self) {
        self.frame_cycle += 1;
        match self.frame_cycle {
            7457 | 22371 => self.quarter_frame(),
            14913 | 29829 => {
                self.quarter_frame();
                self.half_frame();
            }
            29830 => self.frame_cycle = 0,
            _ => {}
        }
    }

    fn quarter_frame(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.noise.envelope.clock();
        self.triangle.clock_linear();
    }

    fn half_frame(&mut self) {
        self.pulse1.length.clock();
        self.pulse2.length.clock();
        self.triangle.length.clock();
        self.noise.length.clock();
        self.pulse1.clock_sweep();
        self.pulse2.clock_sweep();
    }

    /// 4 つのチャンネルを混ぜる（本物の回路に近い計算式。出力は 0.0〜1.0 くらい）
    fn mix(&self) -> f32 {
        let pulse = (self.pulse1.output() + self.pulse2.output()) as f32;
        let triangle = self.triangle.output() as f32;
        let noise = self.noise.output() as f32;
        let pulse_out = if pulse == 0.0 {
            0.0
        } else {
            95.88 / (8128.0 / pulse + 100.0)
        };
        let tnd = triangle / 8227.0 + noise / 12241.0;
        let tnd_out = if tnd == 0.0 {
            0.0
        } else {
            159.79 / (1.0 / tnd + 100.0)
        };
        pulse_out + tnd_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_counter() {
        let mut apu = Apu::new();
        // $4015 で有効にしていないチャンネルは、長さを書いても 0 のまま
        apu.write_register(0x4003, 0x08);
        assert_eq!(apu.pulse1.length.value, 0);
        apu.write_register(0x4015, 0x01);
        apu.write_register(0x4000, 0x10); // 長さカウンタは止めない、一定音量 0
        apu.write_register(0x4003, 0x08); // 長さ = LENGTH_TABLE[1] = 254
        assert_eq!(apu.pulse1.length.value, 254);
        // 1/120 秒ごとに 1 減る: 254 回で 0
        for _ in 0..254 {
            apu.half_frame();
        }
        assert_eq!(apu.pulse1.length.value, 0);
    }

    #[test]
    fn produces_samples_at_sample_rate() {
        let mut apu = Apu::new();
        apu.set_sample_rate(48_000.0);
        for _ in 0..(CPU_CLOCK as usize / 10) {
            apu.step();
        }
        // 0.1 秒分で 4800 サンプルくらい
        assert!(
            (4790..=4810).contains(&apu.samples.len()),
            "{}",
            apu.samples.len()
        );
    }
}
