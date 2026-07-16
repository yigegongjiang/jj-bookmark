import AppKit

// AppKit 纯源码入口：手动装配 NSApplication，无 Storyboard / @NSApplicationMain。
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular) // 进 Dock、有主菜单、可聚焦
app.run()
