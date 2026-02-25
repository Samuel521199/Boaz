/**
 * 旁路抓包：通过 tcpdump（Linux/macOS）采集一段时间内的流量摘要，供 rules_engine 分析。
 * 需本机已安装 tcpdump；Windows 下请使用 WSL 或安装 Npcap + 配套工具。
 */

const { spawn } = require('child_process');
const { EOL } = require('os');

const DEFAULT_DURATION_SEC = 60;
const DEFAULT_IFACE = 'any';

/**
 * 解析 tcpdump 单行（-n 不解析域名），提取 src, dst, dport, proto。
 * 示例: 10.0.0.1.12345 > 192.168.1.1.443: Flags [S], ...
 */
function parseTcpdumpLine(line) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith('listening') || trimmed.startsWith('tcpdump')) return null;
  const match = trimmed.match(/^(\S+)\s+>\s+(\S+):\s+/);
  if (!match) return null;
  const [, src, dst] = match;
  const srcParts = src.split('.');
  const dstParts = dst.split('.');
  const dport = dstParts.length >= 5 ? parseInt(dstParts[dstParts.length - 1], 10) : null;
  const proto = trimmed.includes('Flags') ? 'tcp' : trimmed.includes('UDP') ? 'udp' : 'other';
  return {
    ts: Date.now(),
    src: src.trim(),
    dst: dst.trim(),
    dport: Number.isFinite(dport) ? dport : null,
    proto,
  };
}

/**
 * 运行 tcpdump 指定秒数，返回解析后的流摘要数组（去重简化）。
 * @param {number} durationSec
 * @param {string} iface
 * @returns {Promise<Array<{ ts, src, dst, dport, proto }>>}
 */
function capture(durationSec = DEFAULT_DURATION_SEC, iface = DEFAULT_IFACE) {
  return new Promise((resolve, reject) => {
    const args = ['-i', iface, '-n', '-c', '5000', '-l', 'tcp or udp'];
    const child = spawn('tcpdump', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    const flows = [];
    const seen = new Set();
    let stderr = '';

    const timeout = setTimeout(() => {
      child.kill('SIGTERM');
    }, durationSec * 1000);

    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      chunk.split(EOL).forEach((line) => {
        const flow = parseTcpdumpLine(line);
        if (flow && flow.dport != null) {
          const key = `${flow.src}:${flow.dst}:${flow.dport}:${flow.proto}`;
          if (!seen.has(key)) {
            seen.add(key);
            flows.push(flow);
          }
        }
      });
    });

    child.stderr.on('data', (data) => {
      stderr += data.toString();
    });

    child.on('close', (code, signal) => {
      clearTimeout(timeout);
      if (code !== 0 && code != null && signal !== 'SIGTERM') {
        reject(new Error(`tcpdump 退出 ${code}: ${stderr.slice(0, 500)}`));
      } else {
        resolve(flows);
      }
    });

    child.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });
  });
}

module.exports = { capture, parseTcpdumpLine, DEFAULT_DURATION_SEC, DEFAULT_IFACE };
