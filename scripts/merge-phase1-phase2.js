#!/usr/bin/env node
/**
 * 合并阶段一（离线审计）与阶段二（网络审计）报告，输出综合 status 与推送到 Lark（可选）。
 * 用法:
 *   node merge-phase1-phase2.js phase1.json [phase2.json]
 *   cat phase1.json | node merge-phase1-phase2.js
 * 若只传 phase1，则综合报告仅包含阶段一；若传 phase2 或通过 LARK_WEBHOOK_URL 推送则需 phase2 存在或留空。
 * 环境变量: LARK_WEBHOOK_URL 若设置则最后 POST 合并后的 Markdown 到飞书。
 */

const fs = require('fs');
const https = require('https');

const phase1Path = process.argv[2];
const phase2Path = process.argv[3];

function readStdin() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (c) => chunks.push(c));
    process.stdin.on('end', () => resolve(chunks.join('')));
    process.stdin.on('error', reject);
  });
}

function loadPhase1() {
  if (phase1Path) return Promise.resolve(fs.readFileSync(phase1Path, 'utf8'));
  return readStdin();
}

function loadPhase2() {
  if (!phase2Path || !fs.existsSync(phase2Path)) return Promise.resolve(null);
  return Promise.resolve(fs.readFileSync(phase2Path, 'utf8'));
}

function merge(phase1, phase2) {
  const p1 = phase1.status || 'UNKNOWN';
  const p2 = phase2 ? (phase2.status || 'GREEN') : 'GREEN';
  const status =
    p1 === 'RED' || p2 === 'RED' ? 'RED' : p1 === 'YELLOW' || p2 === 'YELLOW' ? 'YELLOW' : 'GREEN';
  return {
    status,
    phase1: {
      status: p1,
      kernel_integrity: phase1.kernel_integrity,
      suspicious_run_keys_count: (phase1.suspicious_run_keys || []).length,
      risky_services_count: (phase1.risky_services || []).length,
      risky_scheduled_tasks_count: (phase1.risky_scheduled_tasks || []).length,
      yara_matches_count: (phase1.yara_matches || []).length,
      esp_integrity: phase1.esp_integrity,
      esp_missing: (phase1.esp_integrity || []).some((e) => (e.sha256 || '') === 'FILE_NOT_FOUND'),
    },
    phase2: phase2
      ? {
          status: p2,
          source: phase2.source,
          duration_sec: phase2.duration_sec,
          alerts_count: (phase2.alerts || []).length,
          summary: phase2.summary,
          dangerous_ports: (phase2.summary && phase2.summary.dangerousPorts) || 0,
        }
      : null,
    timestamp: new Date().toISOString(),
  };
}

function toMarkdown(merged) {
  const e = merged.status === 'RED' ? '🔴' : merged.status === 'YELLOW' ? '🟡' : '🟢';
  let md = `# Boaz 综合审计报告\n\n**综合可信度** ${e} **${merged.status}**\n\n`;
  md += `## 阶段一（离线）\n- 状态: ${merged.phase1.status}\n`;
  md += `- 内核: ${(merged.phase1.kernel_integrity && merged.phase1.kernel_integrity.sha256) || 'N/A'}\n`;
  md += `- 可疑 Run: ${merged.phase1.suspicious_run_keys_count}, 风险服务: ${merged.phase1.risky_services_count || 0}, 风险计划任务: ${merged.phase1.risky_scheduled_tasks_count || 0}, Yara 命中: ${merged.phase1.yara_matches_count}\n`;
  if (merged.phase1.esp_missing) md += `- ESP 引导: 存在缺失文件\n`;
  md += '\n';
  if (merged.phase2) {
    md += `## 阶段二（网络）\n- 状态: ${merged.phase2.status}\n`;
    md += `- 数据源: ${merged.phase2.source || 'tcpdump'}, 抓包时长: ${merged.phase2.duration_sec}s, 告警数: ${merged.phase2.alerts_count}\n`;
    if (merged.phase2.summary) {
      md += `- 总流数: ${merged.phase2.summary.total}, 危险端口连接: ${merged.phase2.dangerous_ports ?? merged.phase2.summary.dangerousPorts ?? 0}, 非常规端口: ${merged.phase2.summary.unusualPorts}\n`;
    }
    md += '\n';
  }
  return md;
}

function postLark(body) {
  const url = process.env.LARK_WEBHOOK_URL;
  if (!url || !url.startsWith('https://')) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const u = new URL(url);
    const payload = JSON.stringify({ msg_type: 'text', content: { text: body } });
    const req = https.request(
      {
        hostname: u.hostname,
        port: 443,
        path: u.pathname + u.search,
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(payload) },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => resolve(Buffer.concat(chunks).toString()));
      }
    );
    req.on('error', reject);
    req.write(payload);
    req.end();
  });
}

async function main() {
  const raw1 = await loadPhase1();
  let phase1;
  try {
    phase1 = JSON.parse(raw1);
  } catch (e) {
    console.error('阶段一 JSON 解析失败:', e.message);
    process.exit(1);
  }

  const raw2 = await loadPhase2();
  let phase2 = null;
  if (raw2) {
    try {
      phase2 = JSON.parse(raw2);
    } catch (_) {}
  }

  const merged = merge(phase1, phase2);
  console.log(JSON.stringify(merged, null, 2));

  const md = toMarkdown(merged);
  if (process.env.LARK_WEBHOOK_URL) {
    try {
      await postLark(md);
      console.error('已推送至 Lark');
    } catch (e) {
      console.error('推送 Lark 失败:', e.message);
    }
  }
}

main();
