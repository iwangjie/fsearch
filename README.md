# FSearch — macOS 定制优化版

`iwangjie/fsearch` 维护的定制分支，基于上游
[noahdunnagan/fsearch](https://github.com/noahdunnagan/fsearch)。
上游的能力照旧，改动见下面一节。

Whole-disk file search for macOS. Finds any file by name in about a
millisecond, forgives typos, and searches inside files with an index. Use it
as a CLI (with a small daemon) or as a Rust crate.

```
cargo build --release && ./target/release/fsearch install   # -> ~/.local/bin/fsearch
fsearch fsearch main              # find files by name
fsearch 'ext:rs grep:apply_dir'   # search inside files
```

## 相对上游的改动

- **忽略规则**：属于机器的子树在打开之前就跳过，不再产生事件与索引条目——缓存、日志、
  `/private/var`、卷元数据、索引自己的目录，外加可配置的名字规则（`.DS_Store`、`Caches` 等）。
  规则文件：`~/Library/Application Support/FSearch/ignore`，`!` 可反撤销。见下面 "Ignoring"。
- **`install` 不再删掉正在运行的自己**：改成写到目标旁边的临时文件再 `rename()`；从已安装的副本
  执行 `install` 是幂等空操作（原来会先 `remove_file` 自己，再从已被删除的路径 `copy`，必然 ENOENT）。
- **登录项不再空转重启**：`install --login` 生成的 plist 用 `serve --wait`。抢不到 socket 的 daemon
  改为等待接管，而不是退出——原来退出会让 launchd 的 `KeepAlive` 每 10 秒重启一次，日志刷屏。
- **未就绪提示区分两种状态**：首次建索引（`first run scans the whole disk`）与装载已有索引
  （`loading the index and replaying changes`）。

## Speed

M4 Max, 7.7M files and folders on disk.

| | |
|---|---|
| find a file by name, whole disk | p50 1.3 ms |
| search inside files | p50 9 ms |
| a new, renamed or deleted file shows up | ~0.1 s |
| first crawl of the disk | ~20 s, once |
| daemon memory | 30-135 MB |

## vs fff

Chromium (509k files), same Mac, same queries. Video:
[`demo/fsearch-vs-fff.mp4`](demo/fsearch-vs-fff.mp4), method:
[`demo/vs_fff.py`](demo/vs_fff.py).

| | fsearch | [fff](https://github.com/dmtrKovalenko/fff) |
|---|---|---|
| find a file by name | 1.1 ms | 13.8 ms |
| search inside files | 5.6 ms | 53 ms |
| typo still finds the file first | 98% | 88% |
| ready after launch | 50 ms | 2.5 s |
| memory | 50 MB (whole disk) | 358 MB (that folder) |

On the smaller Linux kernel (96k files), name search is a tie and fsearch
wins the rest. fff searches the contents of about 9% more files, because
fsearch skips some file types and `build/` and `vendor/` folders.

## Queries

```
fsearch 'readme in:~/Developer'          # inside a folder
fsearch 'type:image size:>5mb mtime:<7d'
fsearch 'ext:rs regex:fn\s+\w+_dir'      # regex inside files
fsearch 'sym:apply_dir'                  # where it's defined
```

Words are fuzzy, and 5+ letter words forgive one typo (`mian.rs` finds
`main.rs`). Also `'exact`, `^prefix`, `suffix$` and `!exclude`. Filters:
`ext:` `type:` `kind:` `in:` `size:` `mtime:` `re:` `path:` `grep:` `regex:`
`sym:` `limit:`. Content search is smart-case.

## Full Disk Access

Started from a terminal with Full Disk Access, it indexes everything. As a
login item (`fsearch install --login`), give `~/.local/bin/fsearch` its own
grant in System Settings > Privacy & Security, again after each rebuild.
Without access it skips the protected folders instead of popping a prompt.

## Ignoring

Indexing costs CPU per change event, not per entry, so subtrees that belong to
the machine — caches, logs, `/private/var`, volume metadata, and the index's own
directory — are skipped before they are ever opened. Add your own rules in
`~/Library/Application Support/FSearch/ignore`:

```
~/work/scratch        a subtree, home-relative
/opt/big-thing        a subtree, absolute
node_modules          any file or directory with this name, at any depth
!~/Library/Caches/x   take a rule back, or carve a hole in an ignored tree
```

`#` starts a comment. Restart the daemon to pick the file up (`pkill -f
'fsearch serve'`; it comes back on the next query, or launchd restarts it when
installed with `--login`). Entries already in the index go away as their
directories change, or immediately after deleting `index.bin` and `content/` for
a fresh crawl.

## API

JSON lines over `~/Library/Application Support/FSearch/fsearch.sock`, or
`fsearch stdio`:

```json
{"q": "fsearch main", "limit": 20}
{"op": "grep", "pattern": "apply_dir", "in": "~/Developer"}
```

Or link the crate:

```rust
let engine = fsearch::Engine::start(fsearch::Options { dir: fsearch::default_dir(&home), home: home.clone(), skip: None })?;
let hits = engine.search(&fsearch::Query::parse("fsearch main", &home)?)?;
```

An app and the CLI share one index: the first process owns it and the
others follow along.

## How it works

- Crawls the disk once with `getattrlistbulk`, then stays current from
  FSEvents. A restart replays only what changed.
- Names live in one mmap'd file, laid out folder by folder so `in:` is a
  range. Each distinct name is scored once.
- Content search uses a trigram index of your text files. Matches are read
  fresh from disk, so they're never stale.
