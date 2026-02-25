/**
 * 规则引擎单元测试：危险端口、非常规端口、高频告警逻辑。
 * 运行: node test/rules_engine.test.js
 */
const { analyze, DANGEROUS_PORTS, WELL_KNOWN_PORTS } = require('../src/rules_engine.js');

function assert(cond, msg) {
  if (!cond) throw new Error(msg || 'assertion failed');
}

// 危险端口应触发 dangerous_port 告警
const flowsDangerous = [
  { ts: 1, src: '1.2.3.4', dst: '5.6.7.8', dport: 4444, proto: 'tcp' },
  { ts: 2, src: '1.2.3.4', dst: '5.6.7.9', dport: 1337, proto: 'tcp' },
];
const out1 = analyze(flowsDangerous);
assert(out1.alerts.some((a) => a.type === 'dangerous_port'), '应有 dangerous_port 告警');
assert(out1.summary.dangerousPorts === 2, 'dangerousPorts 应为 2');

// 仅常规端口不应有危险端口告警
const flowsSafe = [
  { ts: 1, src: '1.2.3.4', dst: '5.6.7.8', dport: 443, proto: 'tcp' },
  { ts: 2, src: '1.2.3.4', dst: '5.6.7.8', dport: 80, proto: 'tcp' },
];
const out2 = analyze(flowsSafe);
assert(!out2.alerts.some((a) => a.type === 'dangerous_port'), '仅 80/443 不应有 dangerous_port');

// 非常规端口数量 < 5 时不告警
const flowsUnusualFew = [
  { ts: 1, src: '1.2.3.4', dst: '5.6.7.8', dport: 12345, proto: 'tcp' },
  { ts: 2, src: '1.2.3.4', dst: '5.6.7.8', dport: 12346, proto: 'tcp' },
];
const out3 = analyze(flowsUnusualFew);
assert(!out3.alerts.some((a) => a.type === 'unusual_port'), '非常规端口 < 5 不应告警');

// 非常规端口 >= 5 时告警
const flowsUnusualMany = Array.from({ length: 6 }, (_, i) => ({
  ts: i,
  src: '1.2.3.4',
  dst: '5.6.7.8',
  dport: 20000 + i,
  proto: 'tcp',
}));
const out4 = analyze(flowsUnusualMany);
assert(out4.alerts.some((a) => a.type === 'unusual_port'), '非常规端口 >= 5 应有告警');

// 常量存在
assert(DANGEROUS_PORTS.has(4444) && DANGEROUS_PORTS.has(1337), 'DANGEROUS_PORTS 应含 4444、1337');
assert(WELL_KNOWN_PORTS.has(443) && WELL_KNOWN_PORTS.has(80), 'WELL_KNOWN_PORTS 应含 80、443');

console.log('rules_engine 测试全部通过');
process.exit(0);
