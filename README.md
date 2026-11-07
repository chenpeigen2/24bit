# twentyfourbit

非官方的 [24bit.net](https://www.24bit.net)（Hi-Res 无损音乐直链下载站）Rust 客户端。三个 crate 组成的 workspace：

| crate | 作用 |
|-------|------|
| `twentyfourbit-core` | HTTP API 客户端：搜索、详情、下载（支持断点续传）、周榜、登录、配置 |
| `twentyfourbit-player` | 音频播放：symphonia 解码 → 采样率规划/重采样 → cpal 输出 |
| `twentyfourbit-cli` | 命令行工具，二进制名 `24bit` |

## 安装 / 构建

```sh
cargo build --release
# 二进制在 target/release/24bit
```

## CLI 用法

```sh
# 搜索（自动合并两个曲库并去重）
24bit search 富士山下
24bit search 周杰伦 --limit 5 --json

# 周榜
24bit charts

# 查看曲目详情（含签名下载直链及其过期时间）
24bit track c48ed2182ee4c5554d1def46d1aacbc8

# 下载（默认至臻母带 24bit/192kHz FLAC）
24bit download c48ed2182ee4c5554d1def46d1aacbc8 -o ~/Music/foo.flac
24bit download <id> --quality surround   # 高清环绕声，文件更小

# 播放（自动重采样到设备支持的最高采样率）
24bit play <id>
24bit play <id> --keep   # 播放后保留临时文件

# 登录（可选；游客每天可下载数首）
24bit login <account> --password <password>
24bit login <account>   # 从 TWENTYFOURBIT_PASSWORD 环境变量读取

# 配置
24bit config show
24bit config set-quality surround
24bit config path
```

配置文件在 `~/.config/twentyfourbit/config.toml`：

```toml
quality = "master"        # 或 "surround"
token = "..."             # 登录后自动写入
ip_placeholder = "0.0.0.0"
# download_dir = "/Users/you/Music/24bit"
```

## 库用法

```rust,ignore
use twentyfourbit_core::{Client, Quality};

let client = Client::new()?;
let hits = client.search("富士山下").await?;
let d = client.track(&hits[0].id, Quality::Master).await?;
client.download_to(&d.download_url, "out.flac".as_ref()).await?;
```

## 站点 API（逆向所得，无官方文档）

全部端点：`POST https://www.24bit.net/api/<path>`，JSON body，响应为
`{"status": bool, "result": ...}`。

| 端点 | body | 说明 |
|------|------|------|
| `/player/searchOnlineMusicOne` | `{keyword, page}` | 曲库一，30 条/页 |
| `/player/searchOnlineMusicTwo` | `{keyword, page}` | 曲库二（与一不同） |
| `/music/findMusicPage` | `{type, id, ip}` | 详情+签名直链；`type` 为 `a`(母带)/`c`(环绕声)；`ip` 必填但值不校验 |
| `/player/findWeekRecord` | `{}` | 周榜（musics + players） |
| `/user/login` | `{account, password}` | 返回 token，放 `Token` header |
| `/user/getUserStatus` | — | 随 `Token` header 查询 |
| `/music/getTodayMusicCount` | — | 今日已下载次数 |

下载直链指向 `iot*.music.126.net`，路径中带 `YYYYMMDDhhmmss` 过期戳，
过期后返回 403（`Error::ExpiredLink`，重新取详情即可）。支持 Range 续传。

**已知的坑（本 crate 已处理）：**

1. CDN 把 FLAC 标成 `audio/mpeg` —— 扩展名必须取自 `format` 字段。
2. `findMusicPage` 强制要求 `ip` 字段，但值随便填。
3. 站点会间歇性重置连接 —— 客户端默认重试 3 次（5xx/429/网络错误才重试，`status:false` 的业务拒绝不重试）。
4. `searchOnlineMusicThree` 已失效（超时），不要用。
5. 母带档 `type=b` 服务端 500，同样不要用。

## 测试

```sh
cargo test          # 46 个测试，全部离线（wiremock + 真实响应 fixtures）
```

测试期望值全部来自抓取的真实服务器响应（`twentyfourbit-core/tests/fixtures/`），
不与实现互为真值。播放器的时序测试需要声卡（CI 无声卡环境会跳过失败）。

## License

MIT OR Apache-2.0
