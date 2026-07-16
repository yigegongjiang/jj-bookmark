import AppKit
import Foundation

// 闲置自动退出：低频工具打开 link 后常被遗忘在后台，倒计时到点自动 exit。
// idle-timer（有意义交互 / 重新激活 → 重置），覆盖「前台发呆」与「切走遗忘」两种场景。
// 启动即 arm（不等首次交互，否则「开了就走、一次没点」永不退出）。
@MainActor
final class AutoExitManager {
    static let enabledKey = "autoExitEnabled"
    static let minutesKey = "autoExitMinutes"
    static let defaultMinutes = 1
    static let presets = [1, 5, 10]  // Settings 预设；其余为自定义

    // 默认开启：需求即「打开后自动退出」，default off 等于默认不生效。
    static var isEnabled: Bool {
        get {
            UserDefaults.standard.object(forKey: enabledKey) == nil
                ? true : UserDefaults.standard.bool(forKey: enabledKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

    static var minutes: Int {
        get {
            let v = UserDefaults.standard.integer(forKey: minutesKey)
            return v > 0 ? v : defaultMinutes
        }
        set { UserDefaults.standard.set(max(1, newValue), forKey: minutesKey) }
    }

    // 倒计时秒数：分钟换算。
    static var intervalSeconds: TimeInterval { TimeInterval(minutes * 60) }

    private var timer: Timer?
    private var eventMonitor: Any?
    private var activity: NSObjectProtocol?

    func start() {
        // 防 App Nap 节流：核心场景为切后台后仍要触发；不阻止系统 idle 睡眠。
        activity = ProcessInfo.processInfo.beginActivity(
            options: .userInitiatedAllowingIdleSystemSleep,
            reason: "jj-bookmark 闲置自动退出倒计时")
        // local monitor：仅本 App 前台交互重置，后台不重置（符合预期）、免 accessibility 授权。
        eventMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel]
        ) { [weak self] event in
            self?.reset()
            return event
        }
        NotificationCenter.default.addObserver(
            self, selector: #selector(reset),
            name: NSApplication.didBecomeActiveNotification, object: nil)
        reset()
    }

    // 设置变更后调用：按最新开关/时长重排倒计时。
    func reload() { reset() }

    @objc func reset() {
        timer?.invalidate()
        timer = nil
        guard Self.isEnabled else { return }
        let interval = Self.intervalSeconds
        // .common 模式：菜单/弹窗等 tracking loop 期间也计时。
        let t = Timer(timeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor in self?.fire() }
        }
        t.tolerance = min(interval * 0.1, 30)
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    private func fire() {
        // 有模态/sheet 时可能正在编辑，推迟一轮避免打断。
        if NSApp.modalWindow != nil
            || NSApp.windows.contains(where: { $0.attachedSheet != nil }) {
            reset()
            return
        }
        NSApp.terminate(nil)
    }
}
