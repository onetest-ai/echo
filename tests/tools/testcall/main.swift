// TestCall: a two-party audio call between two Macs on the local network, for Echo's functional tests.
//
// Echo (debug build, ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall) treats this app as the call app: a call is
// this app holding a microphone; the remote side is what this app plays. So a test runs Echo's real recording path —
// call detection, the app's audio tap, the mic track, the call ending — with known words and no Teams account.
//
//   testcall serve --port 7766 [--play remote.wav] [--quiet]            wait for one call, send the wav, play what comes
//   testcall dial <host>:7766 [--me mic.wav --me-device "BlackHole 2ch"] [--play x.wav] [--quiet]
//
// --play: what this side says (a wav; 16 kHz mono after conversion), streamed in real time.
// --me / --me-device: what the user says, played into a virtual device (BlackHole) whose input this app opens as its
//   microphone and sends — so Echo records it as "Me" from the device the call app uses. Without --me-device the app
//   opens no microphone (the serving side needs none).
// --quiet: don't play what comes in (the other side's speakers).
// Both sides start together: the server answers HELLO with START <unix ms> 1.5 s ahead; each plays from then. The call
// ends when both have nothing more to send (BYE each way) or the connection drops.
//
// Protocol (TCP): text lines first — "TESTCALL 1\n" from the dialer, "START <ms>\n" from the server — then frames:
// one type byte ('A' audio, 'B' bye), a 2-byte little-endian length, the payload (audio: 16 kHz mono s16le).

import AVFoundation
import CoreAudio
import Foundation
import Network

let RATE = 16000.0
let FRAME = 320 // 20 ms
let fmt16k = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: RATE, channels: 1, interleaved: false)!

func die(_ s: String) -> Never { FileHandle.standardError.write((s + "\n").data(using: .utf8)!); exit(1) }
func log(_ s: String) { FileHandle.standardError.write(("testcall: " + s + "\n").data(using: .utf8)!) }

var args = Array(CommandLine.arguments.dropFirst())
func opt(_ name: String) -> String? {
    guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
    return args[i + 1]
}
let quiet = args.contains("--quiet")
guard let mode = args.first, ["serve", "dial"].contains(mode) else {
    die("usage: testcall serve --port P [--play x.wav] [--quiet] | dial host:P [--me mic.wav --me-device NAME] [--play x.wav] [--quiet]")
}

// ── audio helpers ────────────────────────────────────────────────────────────────────────────────────────────

/// A wav as 16 kHz mono float samples.
func load(_ path: String) -> [Float] {
    guard let f = try? AVAudioFile(forReading: URL(fileURLWithPath: path)) else { die("can't read \(path)") }
    let src = AVAudioPCMBuffer(pcmFormat: f.processingFormat, frameCapacity: AVAudioFrameCount(f.length))!
    try! f.read(into: src)
    let conv = AVAudioConverter(from: f.processingFormat, to: fmt16k)!
    let cap = AVAudioFrameCount(Double(f.length) * RATE / f.processingFormat.sampleRate) + 1024
    let out = AVAudioPCMBuffer(pcmFormat: fmt16k, frameCapacity: cap)!
    var done = false
    var err: NSError?
    conv.convert(to: out, error: &err) { _, status in
        if done { status.pointee = .endOfStream; return nil }
        done = true; status.pointee = .haveData; return src
    }
    return Array(UnsafeBufferPointer(start: out.floatChannelData![0], count: Int(out.frameLength)))
}

/// The CoreAudio device with this name, if any.
func device(named name: String) -> AudioDeviceID? {
    var addr = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size)
    var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
    AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &ids)
    for id in ids {
        var n: Unmanaged<CFString>?
        var s = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        var a = AudioObjectPropertyAddress(mSelector: kAudioObjectPropertyName, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        if AudioObjectGetPropertyData(id, &a, 0, nil, &s, &n) == noErr, let n, (n.takeRetainedValue() as String) == name { return id }
    }
    return nil
}

func setDevice(_ unit: AudioUnit?, _ dev: AudioDeviceID) {
    guard let unit else { die("no audio unit") }
    var d = dev
    let r = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &d, UInt32(MemoryLayout<AudioDeviceID>.size))
    if r != noErr { die("can't select the audio device (\(r))") }
}

/// Plays float samples (16 kHz mono) on an output device (default if nil), from `at` (Date) on.
final class Player {
    let engine = AVAudioEngine()
    let node = AVAudioPlayerNode()
    init(device dev: AudioDeviceID?) {
        if let dev { setDevice(engine.outputNode.audioUnit, dev) }
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: fmt16k)
        try! engine.start()
        node.play()
    }
    func play(_ s: ArraySlice<Float>) {
        let b = AVAudioPCMBuffer(pcmFormat: fmt16k, frameCapacity: AVAudioFrameCount(s.count))!
        b.frameLength = AVAudioFrameCount(s.count)
        s.withUnsafeBufferPointer { b.floatChannelData![0].update(from: $0.baseAddress!, count: s.count) }
        node.scheduleBuffer(b)
    }
}

/// Opens an input device as the microphone and hands 20 ms frames (16 kHz mono) to `each`.
final class Mic {
    let engine = AVAudioEngine()
    var pending: [Float] = []
    init(device dev: AudioDeviceID, each: @escaping ([Float]) -> Void) {
        setDevice(engine.inputNode.audioUnit, dev)
        let inFmt = engine.inputNode.outputFormat(forBus: 0)
        let conv = AVAudioConverter(from: inFmt, to: fmt16k)!
        engine.inputNode.installTap(onBus: 0, bufferSize: 1024, format: inFmt) { [weak self] buf, _ in
            guard let self else { return }
            let out = AVAudioPCMBuffer(pcmFormat: fmt16k, frameCapacity: AVAudioFrameCount(Double(buf.frameLength) * RATE / inFmt.sampleRate) + 64)!
            var given = false
            var err: NSError?
            conv.convert(to: out, error: &err) { _, st in
                if given { st.pointee = .noDataNow; return nil }
                given = true; st.pointee = .haveData; return buf
            }
            self.pending += UnsafeBufferPointer(start: out.floatChannelData![0], count: Int(out.frameLength))
            while self.pending.count >= FRAME {
                each(Array(self.pending.prefix(FRAME)))
                self.pending.removeFirst(FRAME)
            }
        }
        try! engine.start()
    }
}

// ── the call ─────────────────────────────────────────────────────────────────────────────────────────────────

let queue = DispatchQueue(label: "testcall")
var conn: NWConnection!
var startAt: Date?
var sentBye = false, gotBye = false
let speaker = quiet ? nil : Player(device: nil)
let say: [Float] = opt("--play").map(load) ?? []
let meSay: [Float] = opt("--me").map(load) ?? []
var mic: Mic?
var meOut: Player?

func send(_ type: UInt8, _ payload: Data = Data()) {
    var d = Data([type, UInt8(payload.count & 0xff), UInt8(payload.count >> 8)])
    d.append(payload)
    conn.send(content: d, completion: .contentProcessed { _ in })
}

func sendAudio(_ f: [Float]) {
    var d = Data(capacity: f.count * 2)
    for x in f {
        let v = Int16(max(-1, min(1, x)) * 32767)
        d.append(UInt8(truncatingIfNeeded: v)); d.append(UInt8(truncatingIfNeeded: v >> 8))
    }
    send(UInt8(ascii: "A"), d)
}

func finishIfDone() {
    if sentBye && gotBye {
        log("call ended")
        queue.asyncAfter(deadline: .now() + 0.5) { conn.cancel(); exit(0) }
    }
}

/// From START on: stream --play in real time; with --me-device, play --me into that device and send its input.
func begin() {
    guard let startAt else { return }
    let wait = max(0, startAt.timeIntervalSinceNow)
    log("starting in \(String(format: "%.1f", wait)) s")
    queue.asyncAfter(deadline: .now() + wait) {
        var total = say.count
        if let name = opt("--me-device") {
            guard let dev = device(named: name) else { die("no audio device named \(name)") }
            mic = Mic(device: dev) { frame in queue.async { sendAudio(frame) } }
            if !meSay.isEmpty { meOut = Player(device: dev); meOut!.play(meSay[...]) }
            total = max(total, meSay.count)
        }
        var i = 0
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now(), repeating: .milliseconds(20))
        timer.setEventHandler {
            if mic == nil, i < say.count {
                sendAudio(Array(say[i..<min(i + FRAME, say.count)]))
            } else if mic != nil, i < say.count {
                // Both a wav and a microphone: the wav goes out as well (mixed by the receiver's player).
                sendAudio(Array(say[i..<min(i + FRAME, say.count)]))
            }
            i += FRAME
            if i >= total + Int(RATE) && !sentBye {
                sentBye = true
                send(UInt8(ascii: "B"))
                timer.cancel()
                finishIfDone()
            }
        }
        timer.resume()
    }
}

var inbox = Data()
var lines = true
func receive() {
    conn.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, complete, err in
        if let data { inbox.append(data); handle() }
        if complete || err != nil { log("the other side hung up"); exit(0) }
        receive()
    }
}

func handle() {
    while lines, let nl = inbox.firstIndex(of: 10) {
        let line = String(decoding: inbox[inbox.startIndex..<nl], as: UTF8.self)
        inbox.removeSubrange(inbox.startIndex...nl)
        if line.hasPrefix("TESTCALL") && mode == "serve" {
            let at = Date().addingTimeInterval(1.5)
            conn.send(content: "START \(Int(at.timeIntervalSince1970 * 1000))\n".data(using: .utf8), completion: .contentProcessed { _ in })
            startAt = at; lines = false; begin()
        } else if line.hasPrefix("START "), mode == "dial", let ms = Double(line.dropFirst(6)) {
            startAt = Date(timeIntervalSince1970: ms / 1000); lines = false; begin()
        }
    }
    while !lines, inbox.count >= 3 {
        let type = inbox[inbox.startIndex]
        let len = Int(inbox[inbox.startIndex + 1]) | Int(inbox[inbox.startIndex + 2]) << 8
        guard inbox.count >= 3 + len else { return }
        let payload = inbox.subdata(in: (inbox.startIndex + 3)..<(inbox.startIndex + 3 + len))
        inbox.removeSubrange(inbox.startIndex..<(inbox.startIndex + 3 + len))
        if type == UInt8(ascii: "A"), let speaker {
            let samples: [Float] = stride(from: 0, to: payload.count - 1, by: 2).map {
                Float(Int16(bitPattern: UInt16(payload[payload.startIndex + $0]) | UInt16(payload[payload.startIndex + $0 + 1]) << 8)) / 32767
            }
            speaker.play(samples[...])
        } else if type == UInt8(ascii: "B") {
            gotBye = true; finishIfDone()
        }
    }
}

if mode == "serve" {
    let port = NWEndpoint.Port(rawValue: UInt16(opt("--port") ?? "7766") ?? 7766)!
    let listener = try! NWListener(using: .tcp, on: port)
    listener.newConnectionHandler = { c in
        if conn != nil { c.cancel(); return }
        conn = c; c.start(queue: queue); log("call from \(c.endpoint)"); receive()
    }
    listener.start(queue: queue)
    log("waiting for a call on port \(port)")
} else {
    guard args.count > 1, let colon = args[1].lastIndex(of: ":"), let port = NWEndpoint.Port(String(args[1][args[1].index(after: colon)...])) else { die("dial host:port") }
    let host = NWEndpoint.Host(String(args[1][..<colon]))
    conn = NWConnection(host: host, port: port, using: .tcp)
    conn.stateUpdateHandler = { st in
        if case .ready = st { log("connected"); conn.send(content: "TESTCALL 1\n".data(using: .utf8), completion: .contentProcessed { _ in }); receive() }
        if case .failed(let e) = st { die("call failed: \(e)") }
    }
    conn.start(queue: queue)
}
dispatchMain()
