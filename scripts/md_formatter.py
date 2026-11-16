import re
from pathlib import Path

IGNORE_DIRS = {'.git', 'target'}
MAX_WIDTH = 99
BLOCK_START = re.compile(r'#{1,6}|[-*+]|\d{1,9}[.)]|[-*_=]+|[>|<].*|```.*')

def is_block_start(text: str) -> bool:
    return BLOCK_START.fullmatch(text.split(maxsplit=1)[0]) is not None

def wrap_text(text: str) -> list[str]:
    wrapped = []
    current = []

    for word in re.findall(r'\s*\S+', text):
        if not current or len("".join([*current, word]).lstrip()) <= MAX_WIDTH:
            current.append(word)
            continue

        next_line = [word]
        while len(current) > 1 and is_block_start(next_line[0]):
            next_line.insert(0, current.pop())

        if is_block_start(next_line[0]):
            current.extend(next_line)
        else:
            wrapped.append("".join(current).lstrip())
            current = next_line

    if current:
        wrapped.append("".join(current).lstrip())

    return wrapped

def wrap_markdown_file(filepath: Path):
    with open(filepath, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    out_lines = []
    paragraph_buffer = []
    in_code_block = False

    def flush_paragraph():
        if paragraph_buffer:
            text = " ".join(line.strip() for line in paragraph_buffer)
            out_lines.extend(wrap_text(text))
            paragraph_buffer.clear()

    for line in lines:
        line = line.rstrip('\n')
        stripped = line.strip()

        if stripped.startswith("```"):
            flush_paragraph()
            in_code_block = not in_code_block
            out_lines.append(line)
            continue

        if in_code_block:
            out_lines.append(line)
            continue

        if not stripped or line.startswith(('    ', '\t')) or is_block_start(stripped):
            flush_paragraph()
            out_lines.append(line)
            continue

        paragraph_buffer.append(line)

        if line.endswith(('  ', '\\')):
            flush_paragraph()
            if line.endswith('  '):
                out_lines[-1] += '  '

    flush_paragraph()

    with open(filepath, 'w', encoding='utf-8') as f:
        f.write("\n".join(out_lines) + "\n")

if __name__ == '__main__':
    root_dir = Path('.')
    for md_file in root_dir.rglob('*.md'):
        if any(part in IGNORE_DIRS for part in md_file.parts):
            continue
        wrap_markdown_file(md_file)
