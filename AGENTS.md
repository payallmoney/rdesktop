# rdesktop — Agent 工作规则

## 【强制】小更改也必须:更新小版本号 → 再发布

**任何代码更改(无论多小:一个 bug 修复、一行调整)完成后,不允许只提交代码。必须:**

1. **递增 `Cargo.toml` 的 `version`** —— 小更改只加补丁位(1.0.0 → 1.0.1 → 1.0.2 …);
   新功能集合或明显行为变化加次版本位(1.0.x → 1.1.0)。
2. **`cargo build --release`** 通过(先结束正在运行的 rdesktop.exe 再构建)。
3. **提交并推送**(`git commit` + `git push origin master`)。
4. **发布 GitHub Release**:
   - 新版本号 → 用 `tools/publish_github.py` 的同款流程创建 `vX.Y.Z` Release,
     或直接调 GitHub API 建.tag + 上传资产;
   - 资产命名:`rdesktop-vX.Y.Z-win-x64.exe`(取 `target/release/rdesktop.exe`);
   - 上传用 `tools/update_release_asset.py`(改 TAG 后复用,它走直连、不打印令牌)。

**禁止**:把新构建覆盖上传到旧版本号的 Release 资产上——用户看到的版本号必须与
Release tag、exe 内部版本一致。

版本号查询:`grep '^version' Cargo.toml`。
