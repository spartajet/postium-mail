"""从 TS i18n 文件生成 Rust 词条表（一次性迁移脚本）。"""
import re

def parse(path):
    out = []
    section = None
    pending_key = None
    buf = None
    for line in open(path, encoding='utf-8'):
        if pending_key is not None:
            buf += line.strip()
            m = re.match(r'^"(.*)",?$', buf)
            if m:
                out.append((pending_key, m.group(1)))
                pending_key = None
            continue
        m = re.match(r'^\s{2}(\w+): \{', line)
        if m:
            section = m.group(1); continue
        if re.match(r'^\s{2}\}', line):
            section = None; continue
        m = re.match(r'^\s{4}(\w+):\s*"(.*)",?\s*$', line)
        if m and section:
            out.append((f"{section}.{m.group(1)}", m.group(2)))
            continue
        m = re.match(r'^\s{4}(\w+):\s*$', line)
        if m and section:
            pending_key = f"{section}.{m.group(1)}"
            buf = ""
    return out

zh = parse('src/lib/i18n/zh-CN.ts')
en = parse('src/lib/i18n/en-US.ts')
print(f"zh={len(zh)} en={len(en)}")
zk = set(k for k, _ in zh); ek = set(k for k, _ in en)
print('zh-en diff:', zk - ek)
print('en-zh diff:', ek - zk)

def emit(name, pairs):
    lines = [f'pub static {name}: &[(&str, &str)] = &[']
    for k, v in pairs:
        esc = v.replace('\\', '\\\\').replace('"', '\\"')
        lines.append(f'    ("{k}", "{esc}"),')
    lines.append('];')
    return '\n'.join(lines)

with open('crates/postium-ui/src/i18n_tables.rs', 'w', encoding='utf-8') as f:
    f.write('// 由 src/lib/i18n/*.ts 自动生成（迁移脚本解析），勿手工编辑。\n// 键格式：section.key（扁平化两级嵌套）。\n\n')
    f.write(emit('ZH', zh) + '\n\n' + emit('EN', en) + '\n')
print('written')
