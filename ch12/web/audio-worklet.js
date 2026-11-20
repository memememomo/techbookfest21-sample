// AudioWorklet: メインスレッドから届いたサンプルを、届いた順に再生する
class NesAudio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = []; // Float32Array の並び
    this.offset = 0; // queue[0] のどこまで再生したか
    this.queued = 0; // たまっているサンプルの数
    this.port.onmessage = (event) => {
      this.queue.push(event.data);
      this.queued += event.data.length;
      // たまりすぎたら（0.2 秒より多い）、古いものから捨てて遅れを取り戻す
      while (this.queued > sampleRate * 0.2 && this.queue.length > 1) {
        this.queued -= this.queue.shift().length - this.offset;
        this.offset = 0;
      }
    };
  }

  process(inputs, outputs) {
    const out = outputs[0][0];
    for (let i = 0; i < out.length; i++) {
      if (this.queue.length === 0) {
        out[i] = 0; // 足りないときは無音にする
        continue;
      }
      const buffer = this.queue[0];
      out[i] = buffer[this.offset++];
      this.queued--;
      if (this.offset >= buffer.length) {
        this.queue.shift();
        this.offset = 0;
      }
    }
    // ほかのチャンネルにも同じ音を入れる
    for (let ch = 1; ch < outputs[0].length; ch++) {
      outputs[0][ch].set(out);
    }
    return true;
  }
}

registerProcessor("nes-audio", NesAudio);
