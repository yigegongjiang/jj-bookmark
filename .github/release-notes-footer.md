---

### 安装（macOS · Apple Silicon / arm64）

产物未签名 / 未公证，下载解压后先移除隔离属性再打开：

```sh
xattr -dr com.apple.quarantine /path/to/jj-bookmark.app   # App
xattr -d  com.apple.quarantine jj-bookmark-cli-macos-arm64 && chmod +x jj-bookmark-cli-macos-arm64   # CLI
```

校验完整性见 `SHA256SUMS.txt`。
