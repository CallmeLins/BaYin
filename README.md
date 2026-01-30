# 八音 (BaYin)

一款基于 Tauri + React 的跨平台桌面音乐播放器，采用 macOS 风格设计。

## 技术栈

- **桌面框架**: [Tauri 2](https://tauri.app/) (Rust)
- **前端框架**: React 18 + TypeScript
- **构建工具**: Vite
- **UI 组件**: Radix UI + Tailwind CSS 4
- **动画**: Framer Motion
- **状态管理**: React Context
- **路由**: React Router

## 项目结构

```
BaYin/
├── src-tauri/          # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── commands/   # Tauri 命令
│   │   ├── models/     # 数据模型
│   │   ├── utils/      # 工具函数
│   │   └── lib.rs      # 主入口
│   ├── capabilities/   # 权限配置
│   ├── icons/          # 应用图标
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── src-ui/             # 前端子仓库 (React)
│   ├── src/
│   │   ├── components/ # UI 组件
│   │   ├── context/    # 状态管理
│   │   ├── services/   # Tauri 服务层
│   │   ├── hooks/      # 自定义 Hooks
│   │   └── routes.ts   # 路由配置
│   ├── package.json
│   └── vite.config.ts
│
└── README.md
```

## 功能特性

### 音乐播放
- 支持多种音频格式 (MP3, FLAC, WAV, AAC, M4A, OGG, WMA, APE, AIFF, DSF, DFF)
- 播放队列管理（添加、移除、清空）
- 多种播放模式（顺序、随机、单曲循环）
- 音量控制与静音
- 进度条拖拽

### 音乐库管理
- 本地文件夹扫描
- 歌曲、专辑、艺术家分类浏览
- 音质标签显示 (HR: Hi-Res, SQ: 无损)
- 歌单创建与管理

### Navidrome 集成
- 连接 Navidrome/Subsonic 服务器
- 远程音乐库浏览与播放
- 歌词同步获取

### 歌词显示
- LRC 格式歌词解析
- 同步滚动高亮
- 支持内嵌歌词和外部 .lrc 文件

### 界面
- macOS 风格设计
- 深色/浅色主题切换
- 响应式布局（支持桌面和移动端）
- 毛玻璃效果

## 开发环境

### 前置要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri 依赖](https://tauri.app/start/prerequisites/)

### 安装依赖

```bash
# 克隆仓库（包含子模块）
git clone --recursive https://github.com/your-repo/BaYin.git

# 或者克隆后初始化子模块
git submodule update --init --recursive

# 安装前端依赖
cd src-ui
npm install
cd ..
```

### 运行开发环境

```bash
npx tauri dev
```

### 构建生产版本

```bash
npx tauri build
```

## 子仓库说明

前端 UI (`src-ui`) 作为 Git 子模块管理，可以独立开发：

```bash
# 独立运行前端（浏览器预览）
cd src-ui
npm run dev
```

在浏览器环境下，Tauri API 会自动降级为 mock 数据。

## Services 层

`src-ui/src/services/` 封装了所有 Tauri 相关调用：

| 模块 | 功能 |
|------|------|
| `audio.ts` | 音频播放控制（支持本地和 Navidrome） |
| `scanner.ts` | 本地音乐扫描、歌词获取 |
| `navidrome.ts` | Navidrome 服务器 API |
| `storage.ts` | 持久化存储 |
| `tauri.ts` | 环境检测工具 |

## 后端命令

`src-tauri/src/commands/` 提供的 Tauri 命令：

| 命令 | 功能 |
|------|------|
| `scan_music_files` | 扫描本地音乐文件 |
| `get_music_metadata` | 获取音频元数据 |
| `get_lyrics` | 获取本地歌词 |
| `test_navidrome_connection` | 测试 Navidrome 连接 |
| `fetch_navidrome_songs` | 获取 Navidrome 歌曲列表 |
| `get_navidrome_stream_url` | 获取 Navidrome 流媒体 URL |
| `get_navidrome_lyrics` | 获取 Navidrome 歌词 |

## 许可证

Apache License
