# Yara 规则目录

把 `.yar` / `.yara` 放这儿（或别的目录），跑 boaz-core 时用 `--rules` 指过去就行，例如：

```bash
boaz-core -m /mnt/windows --rules ./rules
```

会扫挂载盘下 System32、SysWOW64 里的 exe/dll/sys 等，命中会进报告的 `yara_matches` 和 `suggested_removals`。规则可以从 [Yara-Rules](https://github.com/Yara-Rules/rules) 之类的地方拿，自己写的注意别误报太狠。Samuel，2026-02-23。
