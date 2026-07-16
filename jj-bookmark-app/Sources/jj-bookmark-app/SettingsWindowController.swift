import AppKit

// 偏好设置窗口（纯代码，无 xib）。三块：自动退出 / 命令行工具 / 关于·更新。
// 由 AppDelegate 强引用持有（否则窗口一闪即释放、action 失效——AppKit 经典坑）。
@MainActor
final class SettingsWindowController: NSWindowController, NSWindowDelegate {
    private let runner: CLIRunner
    private let autoExit: AutoExitManager

    private let enabledCheck = NSButton(
        checkboxWithTitle: L10n.autoExitCheckbox, target: nil, action: nil)
    private let presetPopup = NSPopUpButton()
    private let customField = NSTextField()
    private let customStepper = NSStepper()
    private let customRow = NSStackView()
    private let customSuffix = NSTextField(labelWithString: L10n.unitMinutes)
    private let cliStatusLabel = NSTextField(labelWithString: "")

    private static let customTag = -1  // 预设外的「自定义」项
    private static let releasesURL = URL(
        string: "https://github.com/yigegongjiang/jj-bookmark/releases/latest")!

    init(runner: CLIRunner, autoExit: AutoExitManager) {
        self.runner = runner
        self.autoExit = autoExit
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 460, height: 320),
            styleMask: [.titled, .closable],
            backing: .buffered, defer: false)
        window.title = L10n.settingsTitle
        super.init(window: window)
        window.delegate = self
        window.contentView = buildContent()
        // 按内容自适应高度，避免固定高裁掉底部控件（内容随文案/控件增减而变）。
        if let cv = window.contentView {
            cv.layoutSubtreeIfNeeded()
            window.setContentSize(cv.fittingSize)
        }
        window.center()
        syncFromDefaults()
        refreshCLIStatus()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) { fatalError("not used") }

    // MARK: - 布局

    private func buildContent() -> NSView {
        let appVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? L10n.valueUnknown

        enabledCheck.target = self
        enabledCheck.action = #selector(toggleEnabled)

        // 预设 1/5/10 分钟 + 自定义。
        for m in AutoExitManager.presets {
            presetPopup.addItem(withTitle: L10n.presetMinutes(m))
            presetPopup.lastItem?.tag = m
        }
        presetPopup.addItem(withTitle: L10n.custom)
        presetPopup.lastItem?.tag = Self.customTag
        presetPopup.target = self
        presetPopup.action = #selector(presetChanged)

        customField.formatter = intFormatter()
        customField.alignment = .right
        customField.target = self
        customField.action = #selector(customEdited)
        customField.widthAnchor.constraint(equalToConstant: 56).isActive = true
        customStepper.minValue = 1
        customStepper.maxValue = 1440
        customStepper.increment = 1
        customStepper.valueWraps = false
        customStepper.target = self
        customStepper.action = #selector(customStepped)
        customRow.orientation = .horizontal
        customRow.spacing = 6
        customRow.alignment = .firstBaseline
        customRow.setViews([customField, customStepper, customSuffix], in: .leading)

        let intervalRow = NSStackView(views: [
            NSTextField(labelWithString: L10n.idleDuration), presetPopup, customRow,
        ])
        intervalRow.orientation = .horizontal
        intervalRow.spacing = 8
        intervalRow.alignment = .firstBaseline

        cliStatusLabel.lineBreakMode = .byWordWrapping
        cliStatusLabel.maximumNumberOfLines = 2
        let reinstallButton = NSButton(
            title: L10n.btnInstallReinstall, target: self, action: #selector(reinstallCLI))
        let updateButton = NSButton(
            title: L10n.btnCheckUpdates, target: self, action: #selector(checkForUpdates))

        let stack = NSStackView(views: [
            sectionHeader(L10n.sectionAutoExit),
            enabledCheck,
            intervalRow,
            hint(L10n.hintAutoExit),
            separator(),
            sectionHeader(L10n.sectionCLI),
            cliStatusLabel,
            reinstallButton,
            separator(),
            sectionHeader(L10n.sectionAbout),
            NSTextField(labelWithString: L10n.currentVersion(appVersion)),
            updateButton,
        ])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 18, left: 20, bottom: 18, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: container.topAnchor),
            stack.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            stack.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
        return container
    }

    private func sectionHeader(_ text: String) -> NSTextField {
        let label = NSTextField(labelWithString: text)
        label.font = .boldSystemFont(ofSize: NSFont.systemFontSize)
        return label
    }

    private func hint(_ text: String) -> NSTextField {
        let label = NSTextField(wrappingLabelWithString: text)
        label.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        label.textColor = .secondaryLabelColor
        label.preferredMaxLayoutWidth = 400
        return label
    }

    private func separator() -> NSBox {
        let box = NSBox()
        box.boxType = .separator
        box.widthAnchor.constraint(equalToConstant: 420).isActive = true
        return box
    }

    private func intFormatter() -> NumberFormatter {
        let f = NumberFormatter()
        f.numberStyle = .none
        f.minimum = 1
        f.maximum = 1440
        f.allowsFloats = false
        return f
    }

    // MARK: - 状态同步

    private func syncFromDefaults() {
        enabledCheck.state = AutoExitManager.isEnabled ? .on : .off
        let m = AutoExitManager.minutes
        customField.integerValue = m
        customStepper.integerValue = m
        // 命中预设选预设项；否则选「自定义」并展开输入框。
        presetPopup.selectItem(withTag: AutoExitManager.presets.contains(m) ? m : Self.customTag)
        updateEnabledStates()
    }

    private var isCustomSelected: Bool { presetPopup.selectedTag() == Self.customTag }

    private func updateEnabledStates() {
        let on = AutoExitManager.isEnabled
        presetPopup.isEnabled = on
        customRow.isHidden = !(on && isCustomSelected)
        customField.isEnabled = on
        customStepper.isEnabled = on
        customSuffix.textColor = on ? .labelColor : .disabledControlTextColor
    }

    private func refreshCLIStatus() {
        let bundle = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? L10n.valueUnknown
        let installed = CLIInstaller.installedVersion() ?? L10n.valueNotInstalled
        cliStatusLabel.stringValue = L10n.cliStatus(bundle: bundle, installed: installed)
    }

    // MARK: - Actions

    @objc private func toggleEnabled() {
        AutoExitManager.isEnabled = (enabledCheck.state == .on)
        updateEnabledStates()
        autoExit.reload()
    }

    @objc private func presetChanged() {
        if isCustomSelected {
            updateEnabledStates()
            applyMinutes(customField.integerValue)  // 保留当前值作为自定义起点
        } else {
            applyMinutes(presetPopup.selectedTag())
            updateEnabledStates()
        }
    }

    @objc private func customEdited() { applyMinutes(customField.integerValue) }
    @objc private func customStepped() { applyMinutes(customStepper.integerValue) }

    private func applyMinutes(_ raw: Int) {
        let m = min(1440, max(1, raw))
        AutoExitManager.minutes = m
        customField.integerValue = m
        customStepper.integerValue = m
        autoExit.reload()
    }

    @objc private func reinstallCLI() {
        CLIInstaller.reinstall(runner: runner)
        refreshCLIStatus()
    }

    @objc private func checkForUpdates() {
        NSWorkspace.shared.open(Self.releasesURL)
    }
}
