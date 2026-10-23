# 磁盘清理（M1）

Windows 磁盘清理工具：快速扫描 / 全盘扫描，树状显示每个文件夹的大小，按风险分级删除。设计方案见 [DESIGN.md](DESIGN.md)。

## 运行

```powershell
npm install
npm run tauri dev                    # 开发模式
npx tauri build --no-bundle          # 生成 src-tauri\target\release\disk-cleaner.exe
npx tauri build                      # 生成安装包（NSIS）
```

## 测试

```powershell
cd src-tauri
cargo test                                                  # 规则、风险判定、确认校验、真实删除（只删测试自己建的临时文件）
cargo test --release real_machine -- --ignored --nocapture  # 在本机实际跑一次快速扫描和 C 盘全盘扫描（只读）
```

## 代码结构

| 文件 | 作用 |
|---|---|
| `src-tauri/rules/builtin.json` | 清理规则库（路径、风险等级、说明），新增规则改这里 |
| `src-tauri/src/rules.rs` | 内置保护名单 + 规则加载与匹配 |
| `src-tauri/src/scanner.rs` | 多线程扫描（不进入链接、云端文件不计大小） |
| `src-tauri/src/tree.rs` | 结果树：风险判定（自上而下）、大小汇总（自下而上） |
| `src-tauri/src/delete.rs` | 删除计划、后端确认校验、执行删除、操作日志 |
| `src-tauri/src/winutil.rs` | 磁盘列表、管理员权限、回收站 |
| `src/` | 界面（树、详情、可清理汇总、分级确认弹窗） |

## 风险等级

| 等级 | 确认方式 | 删除方式 |
|---|---|---|
| 安全 | 一次确认 | 可选永久删除（默认） |
| 需确认 | 一次确认，显示清单 | 回收站 |
| 谨慎（规则明确识别：软件数据、个人文件、聊天记录等，或里面包含这些） | 看影响 → 勾选已了解 → 手动输入名称（禁止粘贴）+ 5 秒倒计时；后端再次校验 | 只能进回收站 |
| 谨慎（只是没有规则能识别） | 看影响 → 勾选已了解；后端再次校验 | 只能进回收站 |
| 受保护 | 不能删除，显示替代做法 | — |

删除记录：`%LOCALAPPDATA%\DiskCleaner\delete-log.jsonl`
