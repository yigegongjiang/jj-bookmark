import CoreServices
import Foundation

// 监听数据目录（非文件）的 FSEvents。原子 rename 会换 inode，故必须监听目录，
// 不能绑定单一 fd/inode（否则 rename 后事件永不触发）。变更 → 去抖合并 → onChange。
@MainActor
final class FSWatcher {
    // nonisolated(unsafe)：stream 仅在主线程读写，但 deinit（非隔离）需访问以停止；
    // 停止后 C 回调不再触发，无并发访问。
    private nonisolated(unsafe) var stream: FSEventStreamRef?
    private let directory: String
    private let onChange: @MainActor () -> Void
    private var debounceTask: Task<Void, Never>?

    init(directory: URL, onChange: @escaping @MainActor () -> Void) {
        self.directory = directory.path
        self.onChange = onChange
    }

    func start() {
        guard stream == nil else { return }
        // 确保目录存在，FSEvents 才有路径可监听（首启可能尚无数据文件）。
        try? FileManager.default.createDirectory(
            atPath: directory, withIntermediateDirectories: true)

        var context = FSEventStreamContext(
            version: 0,
            info: Unmanaged.passUnretained(self).toOpaque(),
            retain: nil, release: nil, copyDescription: nil)
        let flags = UInt32(
            kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagNoDefer)
        guard let stream = FSEventStreamCreate(
            kCFAllocatorDefault,
            fsEventsCallback,
            &context,
            [directory] as CFArray,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            0.1, // 底层延迟；上层再去抖
            flags)
        else { return }

        self.stream = stream
        FSEventStreamSetDispatchQueue(stream, DispatchQueue.global(qos: .utility))
        FSEventStreamStart(stream)
    }

    func stop() {
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
        self.stream = nil
    }

    deinit {
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
    }

    // 由 C 回调（非隔离）调回；hop 到主 actor 并去抖合并连写（合并非丢弃）。
    nonisolated func onRawEvent() {
        Task { @MainActor [weak self] in
            self?.scheduleReload()
        }
    }

    private func scheduleReload() {
        debounceTask?.cancel()
        debounceTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(120))
            if Task.isCancelled { return }
            self?.onChange()
        }
    }
}

// nonisolated 顶层 C 回调：@convention(c) 不能是 actor 隔离的，也不能捕获环境。
// 从 info 取回 FSWatcher 实例。
private nonisolated func fsEventsCallback(
    stream _: ConstFSEventStreamRef,
    info: UnsafeMutableRawPointer?,
    numEvents _: Int,
    eventPaths _: UnsafeMutableRawPointer,
    eventFlags _: UnsafePointer<FSEventStreamEventFlags>,
    eventIds _: UnsafePointer<FSEventStreamEventId>
) {
    guard let info else { return }
    let watcher = Unmanaged<FSWatcher>.fromOpaque(info).takeUnretainedValue()
    watcher.onRawEvent()
}
