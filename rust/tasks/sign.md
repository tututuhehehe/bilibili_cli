# sign 模块任务书

## 目标（验收在此，做到才算完）
把 **appkey 签名与 cookie 处理**移植到 `rust/sign`（3 个函数）。

## 函数清单（行号）
`sign_app`@239 `cookie_header`@271 `_read_jar`@352

## 关键语义与坑
- **`sign_app` 不能录制**（内部 `int(time.time())`，输出随时间变）。方案：
  1. 核心拆成私有函数 `sign_with_ts(params, key, sec, ts) -> (q, md5, urlencoded)`；
  2. `sign_app` = 取当前时钟后调用它；
  3. cargo 测试对 `fixtures/sign_fixed_ts.json`（6 条固定 ts=1700000000 基准，
     含 q / md5 / urlencode 全字段）**逐字节校验**。
- 签名构造：params 副本 + appkey + ts → **按 key 排序** join `k=v` →
  `md5(q+sec)` → urlencode 整个 p（含 sign）。urlencode 必须与 Python
  `urllib.parse.urlencode` 完全一致（百分号大写、空格→`+`、quote_via 默认）。
- `_read_jar`：`#HttpOnly_` 前缀**剥掉再解析**（当注释跳过会丢 SESSDATA——
  README 已知坑）；注释行、过期字段、域名列按源码处理。
  基线给了合成 jar（fixtures/jar1.txt）与不存在文件（错误用例）。
- `cookie_header`：`c.get("cookie") or ""`。

## 基线已录，你要补：
1. `_read_jar`：空文件、只有注释、HttpOnly+过期混合、tab 数不对的畸形行
   （对照源码的容错方式）。
2. `sign_with_ts` 测试：fixtures 全部 6 条 + 你补 unicode 值 / 空 params /
   数字值（int 与 float 的字符串化要与 Python repr 一致——float 陷阱！）。

## 验收标准
1. `diff` 全绿 N/N（cookie_header + _read_jar）；
2. `gate` 全绿；
3. 固定时钟测试全绿；汇报注明「运行时时钟路径未录制，由固定 ts 基准覆盖」。
