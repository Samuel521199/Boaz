#!/usr/bin/env node
/**
 * 把 boaz-core 的审计 JSON 转成 Markdown 推到飞书/Lark。
 * 用法： node push_to_lark.js audit.json  或  boaz-core -m /mnt/windows | node push_to_lark.js
 * 环境变量 LARK_WEBHOOK_URL 填群机器人 Webhook。
 * Samuel, 2026-02-23
 */

const https = require('https');
const { readFileSync } = require('fs');

const LARK_WEBHOOK_URL = process.env.LARK_WEBHOOK_URL;
const MAX_BODY = 18 * 1024; // 飞书约 20K 限制，留余量

function readStdin() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => chunks.push(chunk));
    process.stdin.on('end', () => resolve(chunks.join('')));
    process.stdin.on('error', reject);
  });
}

function buildMarkdown(report) {
  const status = report.status || 'UNKNOWN';
  const statusEmoji = { GREEN: '🟢', YELLOW: '🟡', RED: '🔴' }[status] || '⚪';
  const lines = [
    `# Boaz 离线审计报告`,
    ``,
    `**环境可信度** ${statusEmoji} **${status}**`,
    ``,
    `## 内核完整性`,
    `- 文件: ${report.kernel_integrity?.file ?? 'ntoskrnl.exe'}`,
    `- SHA256: \`${report.kernel_integrity?.sha256 ?? 'N/A'}\`${report.kernel_integrity?.trusted === false ? ' (未在可信库)' : ''}`,
    ``,
  ];

  if (report.core_integrity?.length) {
    lines.push(`## 核心文件完整性`);
    report.core_integrity.slice(0, 8).forEach((c) => {
      lines.push(`- ${escapeMd(c.file)}: \`${escapeMd(c.sha256)}\``);
    });
    lines.push('');
  }
  if (report.esp_integrity?.length) {
    lines.push(`## ESP 引导完整性`);
    report.esp_integrity.forEach((c) => {
      lines.push(`- ${escapeMd(c.file)}: \`${escapeMd(c.sha256)}\``);
    });
    lines.push('');
  }
  if (report.risky_services?.length) {
    lines.push(`## 风险服务 (路径位于可写/非常规目录)`);
    report.risky_services.forEach((s) => {
      lines.push(`- **${escapeMd(s.name)}** → \`${escapeMd(s.image_path)}\``);
    });
    lines.push('');
  }
  if (report.risky_scheduled_tasks?.length) {
    lines.push(`## 风险计划任务 (非系统路径)`);
    report.risky_scheduled_tasks.forEach((t) => {
      lines.push(`- \`${escapeMd(t.path)}\``);
    });
    lines.push('');
  }
  if (report.services?.length || report.scheduled_tasks?.length) {
    lines.push(`## 持久化`);
    if (report.services?.length) lines.push(`- 服务(含 ImagePath): ${report.services.length} 项`);
    if (report.scheduled_tasks?.length) lines.push(`- 计划任务: ${report.scheduled_tasks.length} 项`);
    lines.push('');
  }

  if (report.suspicious_run_keys?.length) {
    lines.push(`## 可疑自启动项 (Run)`);
    report.suspicious_run_keys.forEach((k) => {
      lines.push(`- **${escapeMd(k.name)}** → \`${escapeMd(k.command_path)}\``);
    });
    lines.push('');
  }

  if (report.yara_matches?.length) {
    lines.push(`## Yara 规则命中`);
    report.yara_matches.forEach((m) => {
      lines.push(`- \`${escapeMd(m.path)}\` — 规则: ${escapeMd(m.rule_id)} (${escapeMd(m.namespace)})`);
    });
    lines.push('');
  }

  if (report.suggested_removals?.length) {
    lines.push(`## 建议处置项 (${report.remediation_requested ? '已请求处置' : '仅报告'})`);
    report.suggested_removals.forEach((r, i) => {
      const type = r.type || 'item';
      const desc = r.description || '';
      if (type === 'run_key') {
        lines.push(`${i + 1}. [Run] ${escapeMd(r.name)} — ${escapeMd(r.command_path)} ${desc}`);
      } else if (type === 'file') {
        lines.push(`${i + 1}. [文件] ${escapeMd(r.path)} — ${escapeMd(r.rule_id || '')} ${desc}`);
      } else {
        lines.push(`${i + 1}. ${escapeMd(JSON.stringify(r))}`);
      }
    });
    lines.push('');
  }

  return lines.join('\n');
}

function escapeMd(s) {
  if (s == null) return '';
  return String(s).replace(/\*/g, '\\*').replace(/_/g, '\\_').replace(/`/g, '\\`').replace(/\n/g, ' ');
}

function postToLark(body) {
  return new Promise((resolve, reject) => {
    if (!LARK_WEBHOOK_URL || !LARK_WEBHOOK_URL.startsWith('https://')) {
      reject(new Error('未设置有效 LARK_WEBHOOK_URL 环境变量'));
      return;
    }
    const url = new URL(LARK_WEBHOOK_URL);
    const payload = JSON.stringify({
      msg_type: 'text',
      content: { text: body.length > MAX_BODY ? body.slice(0, MAX_BODY) + '\n\n...(已截断)' : body },
    });
    const opts = {
      hostname: url.hostname,
      port: 443,
      path: url.pathname + url.search,
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(payload) },
    };
    const req = https.request(opts, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const data = Buffer.concat(chunks).toString();
        if (res.statusCode >= 200 && res.statusCode < 300) resolve(data);
        else reject(new Error(`Lark 返回 ${res.statusCode}: ${data}`));
      });
    });
    req.on('error', reject);
    req.write(payload);
    req.end();
  });
}

async function main() {
  let raw;
  const fileArg = process.argv[2];
  try {
    if (fileArg) {
      raw = readFileSync(fileArg, 'utf8');
    } else {
      raw = await readStdin();
    }
  } catch (e) {
    console.error('读取输入失败:', e.message);
    process.exit(1);
  }

  let report;
  try {
    report = JSON.parse(raw);
  } catch (e) {
    console.error('JSON 解析失败:', e.message);
    process.exit(1);
  }

  const markdown = buildMarkdown(report);
  try {
    await postToLark(markdown);
    console.log('已推送至 Lark');
  } catch (e) {
    console.error('推送 Lark 失败:', e.message);
    process.exit(1);
  }
}

main();
