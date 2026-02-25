/**
 * 异常流量判定：优先基于已知 C2/恶意端口做精确告警，辅以非常规端口与高频连接。
 */

const WELL_KNOWN_PORTS = new Set([80, 443, 53, 22, 21, 25, 110, 143, 993, 995, 8080, 8443, 123, 67, 68, 5353]);

/** 常见 C2/后门/远控使用的端口（精确告警用，减少误报） */
const DANGEROUS_PORTS = new Set([
  4444, 5555, 6666, 6667, 8888, 9999, 4443, 1337, 31337, 1234, 5556, 7626,
  10000, 10080, 10081, 27374, 53995, 65432, 12345, 20000, 31338,
]);

/**
 * 分析流列表，返回告警与摘要。优先告警「危险端口」连接，再考虑非常规端口与高频。
 * @param {Array<{ ts, src, dst, dport, proto }>} flows
 * @returns {{ alerts: Array<{ type, message, flows?, targets? }>, summary }}
 */
function analyze(flows) {
  const alerts = [];
  const dangerousFlows = [];
  const unusualPorts = [];
  const dstCount = new Map();

  for (const f of flows) {
    if (f.dport == null) continue;
    dstCount.set(f.dst, (dstCount.get(f.dst) || 0) + 1);
    if (DANGEROUS_PORTS.has(f.dport)) {
      dangerousFlows.push({ ...f });
    } else if (!WELL_KNOWN_PORTS.has(f.dport) && f.dport >= 1 && f.dport <= 65535) {
      unusualPorts.push({ ...f });
    }
  }

  // 1. 精确告警：已知 C2/恶意端口（高置信度）
  if (dangerousFlows.length > 0) {
    alerts.push({
      type: 'dangerous_port',
      message: `检测到 ${dangerousFlows.length} 个已知 C2/后门常用端口连接，建议立即复核`,
      flows: dangerousFlows.slice(0, 30),
    });
  }

  // 2. 非常规端口（仅当数量较多时告警，避免正常软件误报）
  if (unusualPorts.length >= 5) {
    alerts.push({
      type: 'unusual_port',
      message: `检测到 ${unusualPorts.length} 个非常规目标端口连接`,
      flows: unusualPorts.slice(0, 15),
    });
  }

  // 3. 同一目标高频连接：仅当目标端口也为非常规或危险时告警（更精确）
  const highFreqDsts = [...dstCount.entries()].filter(([, n]) => n >= 15);
  if (highFreqDsts.length > 0 && (dangerousFlows.length > 0 || unusualPorts.length >= 5)) {
    alerts.push({
      type: 'high_frequency',
      message: `以下目标连接频次较高，建议复核: ${highFreqDsts.slice(0, 10).map(([d, n]) => `${d}(${n})`).join(', ')}`,
      targets: highFreqDsts.slice(0, 10).map(([dst, count]) => ({ dst, count })),
    });
  }

  return {
    alerts,
    summary: {
      total: flows.length,
      dangerousPorts: dangerousFlows.length,
      unusualPorts: unusualPorts.length,
      uniqueDsts: dstCount.size,
    },
  };
}

module.exports = { analyze, WELL_KNOWN_PORTS, DANGEROUS_PORTS };
