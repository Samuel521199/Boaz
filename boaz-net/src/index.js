/**
 * Boaz 旁路审计网关入口：抓包指定时长后经规则引擎分析，输出 JSON 报告。
 * 用法: node src/index.js [duration_sec] [interface]
 * 例: node src/index.js 60 any
 */

const { capture } = require('./sniffer.js');
const { analyze } = require('./rules_engine.js');
const { snapshotConnections } = require('./windows_snapshot.js');

const durationSec = parseInt(process.argv[2], 10) || 60;
const iface = process.argv[3] || 'any';

async function main() {
  process.stderr.write(`[*] 开始抓包 ${durationSec}s，网卡: ${iface}...\n`);
  let flows;
  let source = 'tcpdump';
  try {
    flows = await capture(durationSec, iface);
  } catch (e) {
    if (process.platform === 'win32') {
      process.stderr.write(`[*] tcpdump 不可用，改用 Windows 连接快照...\n`);
      try {
        flows = snapshotConnections();
        source = 'windows_snapshot';
      } catch (snapErr) {
        process.stderr.write(`[!] 抓包与快照均失败: ${e.message}; ${snapErr.message}\n`);
        process.exit(1);
      }
    } else {
      process.stderr.write(`[!] 抓包失败: ${e.message}\n`);
      process.stderr.write('    请确认已安装 tcpdump 且以足够权限运行（如 sudo）。\n');
      process.exit(1);
    }
  }
  process.stderr.write(`[*] 共采集 ${flows.length} 条流 (${source})，正在分析...\n`);
  const result = analyze(flows);
  const report = {
    status: result.alerts.length > 0 ? 'YELLOW' : 'GREEN',
    duration_sec: source === 'tcpdump' ? durationSec : 0,
    interface: iface,
    source,
    summary: result.summary,
    alerts: result.alerts,
    timestamp: new Date().toISOString(),
  };
  console.log(JSON.stringify(report, null, 2));
}

main().catch((e) => {
  process.stderr.write(String(e));
  process.exit(1);
});
