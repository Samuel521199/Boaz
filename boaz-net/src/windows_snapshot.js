/**
 * Windows 连接快照：在无 tcpdump 时通过 netstat 获取当前 TCP 连接，供规则引擎分析。
 * 输出格式与 sniffer 一致：{ ts, src, dst, dport, proto }。
 */

const { execSync } = require('child_process');
const { EOL } = require('os');

/**
 * 解析 netstat -an 输出行（英文/中文系统均尝试匹配）。
 * 示例: TCP    10.0.0.2:12345    192.168.1.1:443    ESTABLISHED
 * 或:   TCP    10.0.0.2:12345    192.168.1.1:443    已建立
 */
function parseNetstatLine(line) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith('Proto') || trimmed.startsWith('协议')) return null;
  const parts = trimmed.split(/\s+/).filter(Boolean);
  if (parts.length < 4) return null;
  const proto = parts[0].toLowerCase();
  if (proto !== 'tcp' && proto !== 'udp') return null;
  const local = parts[1];
  const remote = parts[2];
  const state = parts[3];
  if (state && (state === 'LISTENING' || state === '监听' || state === 'LISTEN')) return null;
  const parseAddr = (addr) => {
    const lastColon = addr.lastIndexOf(':');
    if (lastColon === -1) return { host: addr, port: null };
    return { host: addr.slice(0, lastColon), port: parseInt(addr.slice(lastColon + 1), 10) };
  };
  const r = parseAddr(remote);
  if (!r.port || r.port < 1 || r.port > 65535) return null;
  const l = parseAddr(local);
  return {
    ts: Date.now(),
    src: local,
    dst: remote,
    dport: r.port,
    proto,
  };
}

/**
 * 执行 netstat -an，解析为流列表（与 sniffer 同格式）。
 * @returns {Array<{ ts, src, dst, dport, proto }>}
 */
function snapshotConnections() {
  let out;
  try {
    out = execSync('netstat -an', { encoding: 'utf8', timeout: 15000, windowsHide: true });
  } catch (e) {
    throw new Error(`netstat 执行失败: ${e.message}`);
  }
  const lines = out.split(EOL);
  const flows = [];
  const seen = new Set();
  for (const line of lines) {
    const flow = parseNetstatLine(line);
    if (flow && flow.dport != null) {
      const key = `${flow.src}:${flow.dst}:${flow.dport}:${flow.proto}`;
      if (!seen.has(key)) {
        seen.add(key);
        flows.push(flow);
      }
    }
  }
  return flows;
}

module.exports = { snapshotConnections, parseNetstatLine };
