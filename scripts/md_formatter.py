import textwrap
from pathlib import Path

IGNORE_DIRS = {'.git', 'target'}
MAX_WIDTH = 100

def wrap_markdown_file(filepath: Path):
    with open(filepath, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    out_lines = []
    paragraph_buffer = []
    in_code_block = False

    def flush_paragraph():
        if paragraph_buffer:
            text = " ".join(line.strip() for line in paragraph_buffer)
            wrapped = textwrap.wrap(text, width=MAX_WIDTH, break_long_words=False)
            out_lines.extend(wrapped)
            paragraph_buffer.clear()

    for line in lines:
        stripped = line.strip()

        if stripped.startswith("```"):
            flush_paragraph()
            in_code_block = not in_code_block
            out_lines.append(line.rstrip('\n'))
            continue

        if in_code_block:
            out_lines.append(line.rstrip('\n'))
            continue

        if not stripped or stripped.startswith(('#', '>', '-', '*', '|', '<')):
            flush_paragraph()
            out_lines.append(line.rstrip('\n'))
            continue

        paragraph_buffer.append(line)

    flush_paragraph()

    with open(filepath, 'w', encoding='utf-8') as f:
        f.write("\n".join(out_lines) + "\n")

if __name__ == '__main__':
    root_dir = Path('.')
    for md_file in root_dir.rglob('*.md'):
        if any(part in IGNORE_DIRS for part in md_file.parts):
            continue
        wrap_markdown_file(md_file)